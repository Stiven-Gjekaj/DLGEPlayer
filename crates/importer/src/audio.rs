//! Converts CRI HCA audio to Ogg Vorbis.
//!
//! Godot 4 loads Ogg Vorbis files at runtime. It does not load Opus. An
//! Ogg Vorbis stream loops from a start point to the end of the stream, so
//! the converter cuts a looped track at its loop end.

use std::fmt;
use std::io::Cursor;
use std::num::{NonZeroU8, NonZeroU32};

use cridecoder::{HcaDecoder, HcaDecoderError};
use vorbis_rs::{VorbisBitrateManagementStrategy, VorbisEncoderBuilder, VorbisError};

/// The number of sample frames that the converter gives to the encoder in
/// one block.
const BLOCK: usize = 1024;

/// The serial number of each Ogg stream. A fixed number makes the output
/// the same for the same input.
const STREAM_SERIAL: i32 = 0x444c_4745;

/// An error from the audio converter.
#[derive(Debug)]
pub enum Error {
    Hca(HcaDecoderError),
    Vorbis(VorbisError),
    /// The HCA file is encrypted. The game uses no encrypted audio.
    Encrypted,
    /// The HCA header gives a value that the encoder cannot use.
    BadHeader(&'static str),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Hca(e) => write!(f, "HCA: {e:?}"),
            Error::Vorbis(e) => write!(f, "Vorbis: {e}"),
            Error::Encrypted => write!(f, "the HCA file is encrypted"),
            Error::BadHeader(what) => write!(f, "bad HCA header: {what}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<HcaDecoderError> for Error {
    fn from(e: HcaDecoderError) -> Error {
        Error::Hca(e)
    }
}

impl From<VorbisError> for Error {
    fn from(e: VorbisError) -> Error {
        Error::Vorbis(e)
    }
}

/// The loop of a track, in sample frames from the start of the output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Loop {
    pub start: u64,
    pub end: u64,
}

/// One converted track.
#[derive(Debug, Clone, PartialEq)]
pub struct Track {
    /// The Ogg Vorbis stream.
    pub ogg: Vec<u8>,
    pub sample_rate: u32,
    pub channels: u32,
    /// The number of sample frames in the output.
    pub frames: u64,
    /// The loop of the track. The output ends at the loop end.
    pub looping: Option<Loop>,
}

/// Converts an HCA file. `quality` is the Vorbis quality from -0.2 to 1.0.
pub fn hca_to_ogg(hca: &[u8], quality: f32) -> Result<Track, Error> {
    let mut decoder = HcaDecoder::from_reader(Cursor::new(hca))?;
    let info = decoder.info().clone();
    if info.encryption_enabled {
        return Err(Error::Encrypted);
    }
    let channels = u8::try_from(info.channel_count)
        .ok()
        .and_then(NonZeroU8::new)
        .ok_or(Error::BadHeader("channel count"))?;
    let rate = NonZeroU32::new(info.sampling_rate).ok_or(Error::BadHeader("sample rate"))?;
    let looping = info.loop_enabled.then(|| {
        let spb = info.samples_per_block as u64;
        let delay = info.encoder_delay as u64;
        Loop {
            start: (info.loop_start_block as u64 * spb + info.loop_start_delay as u64)
                .saturating_sub(delay),
            end: ((info.loop_end_block as u64 + 1) * spb)
                .saturating_sub(info.loop_end_padding as u64)
                .saturating_sub(delay),
        }
    });
    let limit = looping.map_or(u64::MAX, |l| l.end);

    let mut ogg = Vec::new();
    let mut encoder =
        VorbisEncoderBuilder::new_with_serial(rate, channels, &mut ogg, STREAM_SERIAL)
            .bitrate_management_strategy(VorbisBitrateManagementStrategy::QualityVbr {
                target_quality: quality,
            })
            .build()?;

    let count = channels.get() as usize;
    let mut planar: Vec<Vec<f32>> = vec![Vec::with_capacity(BLOCK); count];
    let mut frames = 0u64;
    let mut done = false;
    while !done {
        let (samples, n) = match decoder.decode_frame() {
            Ok(frame) => frame,
            Err(HcaDecoderError::Eof) => break,
            Err(e) => return Err(e.into()),
        };
        let keep = (n as u64).min(limit - frames) as usize;
        for i in 0..keep {
            for (c, channel) in planar.iter_mut().enumerate() {
                channel.push(samples[i * count + c]);
            }
        }
        frames += keep as u64;
        done = frames >= limit;
        if planar[0].len() >= BLOCK || done {
            encoder.encode_audio_block(&planar)?;
            planar.iter_mut().for_each(Vec::clear);
        }
    }
    if !planar[0].is_empty() {
        encoder.encode_audio_block(&planar)?;
    }
    encoder.finish()?;
    Ok(Track {
        ogg,
        sample_rate: info.sampling_rate,
        channels: info.channel_count,
        frames,
        looping,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use cridecoder::{HcaEncoder, HcaEncoderConfig};
    use vorbis_rs::VorbisDecoder;

    /// Builds an HCA file of a sine wave.
    fn hca(rate: u32, channels: u32, frames: usize, loop_points: Option<(u32, u32)>) -> Vec<u8> {
        let mut pcm = Vec::with_capacity(frames * channels as usize);
        for i in 0..frames {
            let v = (i as f32 * 440.0 * std::f32::consts::TAU / rate as f32).sin() * 0.5;
            for _ in 0..channels {
                pcm.push(v);
            }
        }
        let mut config = HcaEncoderConfig::new(rate, channels);
        config.bitrate = 128_000;
        if let Some((start, end)) = loop_points {
            config.loop_start = Some(start);
            config.loop_end = Some(end);
        }
        let mut out = Cursor::new(Vec::new());
        HcaEncoder::new(config)
            .unwrap()
            .encode(&pcm, &mut out)
            .unwrap();
        out.into_inner()
    }

    fn decoded_frames(ogg: &[u8]) -> (u64, u8, u32) {
        let mut decoder = VorbisDecoder::new(Cursor::new(ogg.to_vec())).unwrap();
        let (channels, rate) = (decoder.channels().get(), decoder.sampling_frequency().get());
        let mut frames = 0u64;
        while let Some(block) = decoder.decode_audio_block().unwrap() {
            frames += block.samples()[0].len() as u64;
        }
        (frames, channels, rate)
    }

    #[test]
    fn converts_a_track() {
        let track = hca_to_ogg(&hca(48_000, 2, 48_000, None), 0.3).unwrap();
        assert_eq!(track.sample_rate, 48_000);
        assert_eq!(track.channels, 2);
        assert_eq!(track.frames, 48_000);
        assert_eq!(track.looping, None);
        assert_eq!(decoded_frames(&track.ogg), (48_000, 2, 48_000));
    }

    #[test]
    fn cuts_a_looped_track_at_the_loop_end() {
        let track = hca_to_ogg(&hca(44_100, 1, 44_100, Some((10_000, 30_000))), 0.3).unwrap();
        let looping = track.looping.expect("the track loops");
        assert!(looping.start < looping.end);
        assert!(looping.end <= 44_100);
        assert_eq!(track.frames, looping.end);
        assert_eq!(decoded_frames(&track.ogg).0, looping.end);
    }

    #[test]
    fn gives_the_same_output_for_the_same_input() {
        let input = hca(32_000, 1, 8_000, None);
        assert_eq!(
            hca_to_ogg(&input, 0.3).unwrap().ogg,
            hca_to_ogg(&input, 0.3).unwrap().ogg
        );
    }

    #[test]
    fn rejects_bad_input() {
        assert!(matches!(
            hca_to_ogg(b"not an HCA file", 0.3),
            Err(Error::Hca(_))
        ));
    }
}
