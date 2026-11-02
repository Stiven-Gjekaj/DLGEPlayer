//! Plays the MP4 movies of the import folder.
//!
//! Godot plays only Ogg Theora video, and the movies are H.264 and AAC in
//! MP4. The importer copies them as they are, so the player decodes them:
//! the `mp4` crate reads the samples, `openh264` decodes the pictures, and
//! `symphonia` decodes the sound.

use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use mp4::{MediaType, Mp4Reader, Mp4Sample, TrackType};
use openh264::OpenH264API;
use openh264::decoder::{Decoder, DecoderConfig, Flush};
use openh264::formats::YUVSource;

/// The pictures of one movie.
pub struct Movie {
    reader: Mp4Reader<BufReader<File>>,
    decoder: Decoder,
    track: u32,
    samples: u32,
    /// The number of the next sample to read. The first sample is 1.
    next: u32,
    /// A sample that was read and is not decoded yet.
    pending: Option<Mp4Sample>,
    timescale: f64,
    /// The SPS and the PPS, as Annex B units for the first sample.
    headers: Option<Vec<u8>>,
    /// The bytes of the length before each unit of a sample.
    length_size: usize,
    pub width: usize,
    pub height: usize,
    /// The last picture as RGB, 3 bytes for each pixel.
    pub rgb: Vec<u8>,
    duration: f64,
}

const START: [u8; 4] = [0, 0, 0, 1];

/// Changes the units of a sample from a length before each unit to a start
/// code before each unit (Annex B).
fn annex_b(sample: &[u8], length_size: usize, out: &mut Vec<u8>) {
    let mut at = 0;
    while at + length_size <= sample.len() {
        let mut len = 0usize;
        for b in &sample[at..at + length_size] {
            len = len << 8 | *b as usize;
        }
        at += length_size;
        let end = (at + len).min(sample.len());
        out.extend_from_slice(&START);
        out.extend_from_slice(&sample[at..end]);
        at = end;
    }
}

impl Movie {
    pub fn open(path: &Path) -> Result<Movie, String> {
        let file = File::open(path).map_err(|e| e.to_string())?;
        let size = file.metadata().map_err(|e| e.to_string())?.len();
        let reader =
            Mp4Reader::read_header(BufReader::new(file), size).map_err(|e| e.to_string())?;
        let track = reader
            .tracks()
            .values()
            .find(|t| {
                matches!(t.track_type(), Ok(TrackType::Video))
                    && matches!(t.media_type(), Ok(MediaType::H264))
            })
            .ok_or("the movie has no H.264 track")?;
        let mut headers = Vec::new();
        headers.extend_from_slice(&START);
        headers.extend_from_slice(track.sequence_parameter_set().map_err(|e| e.to_string())?);
        headers.extend_from_slice(&START);
        headers.extend_from_slice(track.picture_parameter_set().map_err(|e| e.to_string())?);
        let length_size = track
            .trak
            .mdia
            .minf
            .stbl
            .stsd
            .avc1
            .as_ref()
            .map(|a| (a.avcc.length_size_minus_one & 3) as usize + 1)
            .unwrap_or(4);
        let width = track.width() as usize;
        let height = track.height() as usize;
        let id = track.track_id();
        let samples = track.sample_count();
        let timescale = track.timescale().max(1) as f64;
        let duration = track.duration().as_secs_f64();
        Ok(Movie {
            reader,
            // Most movies have B-frames. The decoder must keep pictures to
            // give them in order, so it must not flush after each sample.
            decoder: Decoder::with_api_config(
                OpenH264API::from_source(),
                DecoderConfig::new().flush_after_decode(Flush::NoFlush),
            )
            .map_err(|e| e.to_string())?,
            track: id,
            samples,
            next: 1,
            pending: None,
            timescale,
            headers: Some(headers),
            length_size,
            width,
            height,
            rgb: vec![0; width * height * 3],
            duration,
        })
    }

    pub fn duration(&self) -> f64 {
        self.duration
    }

    pub fn finished(&self) -> bool {
        self.next > self.samples && self.pending.is_none()
    }

    /// Decodes the pictures up to the time in seconds. Gives true when
    /// `rgb` holds a new picture.
    pub fn advance(&mut self, seconds: f64) -> Result<bool, String> {
        let mut changed = false;
        loop {
            if self.pending.is_none() {
                if self.next > self.samples {
                    break;
                }
                self.pending = self
                    .reader
                    .read_sample(self.track, self.next)
                    .map_err(|e| e.to_string())?;
                self.next += 1;
            }
            match &self.pending {
                Some(s) if s.start_time as f64 / self.timescale <= seconds => {}
                _ => break,
            }
            let Some(sample) = self.pending.take() else {
                break;
            };
            let mut units = self.headers.take().unwrap_or_default();
            annex_b(&sample.bytes, self.length_size, &mut units);
            if let Some(picture) = self.decoder.decode(&units).map_err(|e| e.to_string())? {
                let (w, h) = picture.dimensions();
                if w * h * 3 != self.rgb.len() {
                    self.width = w;
                    self.height = h;
                    self.rgb = vec![0; w * h * 3];
                }
                picture.write_rgb8(&mut self.rgb);
                changed = true;
            }
        }
        Ok(changed)
    }
}

/// The sound of a movie as 16-bit samples with the channels interleaved.
pub struct Sound {
    pub rate: u32,
    pub channels: usize,
    pub samples: Vec<i16>,
}

/// Decodes the first audio track of a movie. A movie with no audio track
/// gives `None`.
pub fn decode_sound(path: &Path) -> Result<Option<Sound>, String> {
    use symphonia::core::audio::SampleBuffer;
    use symphonia::core::codecs::{CODEC_TYPE_NULL, DecoderOptions};
    use symphonia::core::errors::Error;
    use symphonia::core::formats::FormatOptions;
    use symphonia::core::io::MediaSourceStream;
    use symphonia::core::meta::MetadataOptions;
    use symphonia::core::probe::Hint;

    let file = File::open(path).map_err(|e| e.to_string())?;
    let stream = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    hint.with_extension("mp4");
    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            stream,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .map_err(|e| e.to_string())?;
    let mut format = probed.format;
    let Some(track) = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
    else {
        return Ok(None);
    };
    let id = track.id;
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|e| e.to_string())?;
    let mut sound = Sound {
        rate: track.codec_params.sample_rate.unwrap_or(48000),
        channels: track.codec_params.channels.map(|c| c.count()).unwrap_or(2),
        samples: Vec::new(),
    };
    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(Error::IoError(_)) => break,
            Err(e) => return Err(e.to_string()),
        };
        if packet.track_id() != id {
            continue;
        }
        let decoded = match decoder.decode(&packet) {
            Ok(d) => d,
            // A bad packet loses a short part of the sound only.
            Err(Error::DecodeError(_)) => continue,
            Err(e) => return Err(e.to_string()),
        };
        sound.rate = decoded.spec().rate;
        sound.channels = decoded.spec().channels.count();
        let mut buffer = SampleBuffer::<i16>::new(decoded.capacity() as u64, *decoded.spec());
        buffer.copy_interleaved_ref(decoded);
        sound.samples.extend_from_slice(buffer.samples());
    }
    Ok(Some(sound))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn changes_lengths_to_start_codes() {
        let sample = [0, 0, 0, 2, 0x65, 0xaa, 0, 0, 0, 1, 0x41];
        let mut out = Vec::new();
        annex_b(&sample, 4, &mut out);
        assert_eq!(out, [0, 0, 0, 1, 0x65, 0xaa, 0, 0, 0, 1, 0x41]);
    }

    #[test]
    fn stops_at_a_short_sample() {
        let mut out = Vec::new();
        annex_b(&[0, 0, 0, 9, 1, 2], 4, &mut out);
        assert_eq!(out, [0, 0, 0, 1, 1, 2]);
    }
}
