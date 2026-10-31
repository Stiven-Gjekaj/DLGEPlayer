//! Decodes CRI ADX audio. The menu sounds in `SystemSE.acb` use it.
//!
//! All numbers are big-endian. The header:
//!
//! | Offset | Value |
//! |---|---|
//! | 0x00 | the magic `80 00` |
//! | 0x02 | the copyright offset. The data starts 4 bytes after it. |
//! | 0x04 | the encoding. 3 is the standard ADPCM. |
//! | 0x05 | the frame size, 18 |
//! | 0x06 | the bits for each sample, 4 |
//! | 0x07 | the channel count |
//! | 0x08 | the sample rate, u32 |
//! | 0x0c | the sample count, u32 |
//! | 0x10 | the high-pass frequency, u16 |
//! | 0x12 | the version |
//! | 0x13 | the flags. A value that is not zero means encryption. |
//!
//! The frames of the channels come one after the other. A frame has a
//! scale (u16) and 32 samples of 4 bits, the high nibble first. A sample is
//! `(nibble * scale * 4096 + coef1 * h1 + coef2 * h2) >> 12`, where `h1`
//! and `h2` are the two samples before it in the same channel. The output
//! agrees with the ADX decoder of FFmpeg.

use std::fmt;

const MAGIC: [u8; 2] = [0x80, 0x00];
const STANDARD: u8 = 3;
const FRAME: usize = 18;
const FRAME_SAMPLES: usize = 32;

/// An error from the ADX decoder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    BadHeader(&'static str),
    Unsupported(&'static str),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::BadHeader(what) => write!(f, "bad ADX header: {what}"),
            Error::Unsupported(what) => write!(f, "unsupported ADX file: {what}"),
        }
    }
}

impl std::error::Error for Error {}

/// Decoded audio.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pcm {
    pub sample_rate: u32,
    pub channels: u32,
    /// The samples of all channels, interleaved.
    pub samples: Vec<i16>,
}

/// Gives the two prediction coefficients for a high-pass frequency.
pub fn coefficients(highpass: u16, sample_rate: u32) -> (i32, i32) {
    let z = (2.0 * std::f64::consts::PI * highpass as f64 / sample_rate as f64).cos();
    let a = std::f64::consts::SQRT_2 - z;
    let b = std::f64::consts::SQRT_2 - 1.0;
    let c = (a - ((a + b) * (a - b)).sqrt()) / b;
    (
        (c * 8192.0).round() as i32,
        (c * c * -4096.0).round() as i32,
    )
}

/// Decodes an ADX file.
pub fn decode(data: &[u8]) -> Result<Pcm, Error> {
    if data.len() < 0x14 || data[..2] != MAGIC {
        return Err(Error::BadHeader("magic"));
    }
    let u16_at = |at: usize| u16::from_be_bytes([data[at], data[at + 1]]);
    let u32_at = |at: usize| u32::from_be_bytes(data[at..at + 4].try_into().unwrap());
    if data[4] != STANDARD {
        return Err(Error::Unsupported("encoding"));
    }
    if data[5] as usize != FRAME || data[6] != 4 {
        return Err(Error::Unsupported("frame size"));
    }
    if data[0x13] != 0 {
        return Err(Error::Unsupported("encryption"));
    }
    let channels = data[7] as usize;
    let sample_rate = u32_at(8);
    if channels == 0 || sample_rate == 0 {
        return Err(Error::BadHeader("channel count or sample rate"));
    }
    let total = u32_at(0x0c) as usize;
    let start = u16_at(2) as usize + 4;
    let (coef1, coef2) = coefficients(u16_at(0x10), sample_rate);

    let mut samples = vec![0i16; total * channels];
    let mut history = vec![(0i32, 0i32); channels];
    let frames = total.div_ceil(FRAME_SAMPLES);
    for f in 0..frames {
        for (c, (h1, h2)) in history.iter_mut().enumerate() {
            let at = start + (f * channels + c) * FRAME;
            let frame = data
                .get(at..at + FRAME)
                .ok_or(Error::BadHeader("the data is too short"))?;
            let scale = u16::from_be_bytes([frame[0], frame[1]]) as i32;
            for i in 0..FRAME_SAMPLES {
                let index = f * FRAME_SAMPLES + i;
                if index >= total {
                    break;
                }
                let byte = frame[2 + i / 2];
                let nibble = if i % 2 == 0 { byte >> 4 } else { byte & 0x0f };
                // Sign-extend the 4-bit value.
                let nibble = ((nibble as i8) << 4 >> 4) as i32;
                let sample = ((nibble * scale * 4096 + coef1 * *h1 + coef2 * *h2) >> 12)
                    .clamp(i16::MIN as i32, i16::MAX as i32);
                *h2 = *h1;
                *h1 = sample;
                samples[index * channels + c] = sample as i16;
            }
        }
    }
    Ok(Pcm {
        sample_rate,
        channels: channels as u32,
        samples,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds a file with a header and the given frames.
    fn file(channels: u8, total: u32, frames: &[[u8; FRAME]]) -> Vec<u8> {
        let mut out = MAGIC.to_vec();
        out.extend(0x1cu16.to_be_bytes());
        out.extend([STANDARD, FRAME as u8, 4, channels]);
        out.extend(44_100u32.to_be_bytes());
        out.extend(total.to_be_bytes());
        out.extend(500u16.to_be_bytes());
        out.extend([4, 0]);
        out.resize(0x1a, 0);
        out.extend(b"(c)CRI");
        for f in frames {
            out.extend(f);
        }
        out
    }

    fn frame(scale: u16, nibbles: &[u8]) -> [u8; FRAME] {
        let mut f = [0u8; FRAME];
        f[..2].copy_from_slice(&scale.to_be_bytes());
        for (i, n) in nibbles.iter().enumerate() {
            f[2 + i / 2] |= if i % 2 == 0 { n << 4 } else { n & 0x0f };
        }
        f
    }

    #[test]
    fn gives_known_coefficients() {
        // The values for 500 Hz at 44100 Hz. With them, the decoder gives
        // the same samples as FFmpeg.
        assert_eq!(coefficients(500, 44_100), (7334, -3283));
    }

    #[test]
    fn decodes_with_prediction() {
        let (coef1, coef2) = coefficients(500, 44_100);
        // The scale is 10. The nibbles are 1, -1 (0xf), and 0.
        let pcm = decode(&file(1, 3, &[frame(10, &[1, 0xf, 0])])).unwrap();
        assert_eq!((pcm.sample_rate, pcm.channels), (44_100, 1));
        let s0 = 10;
        let s1 = (-10 * 4096 + coef1 * s0) >> 12;
        let s2 = (coef1 * s1 + coef2 * s0) >> 12;
        assert_eq!(pcm.samples, [s0 as i16, s1 as i16, s2 as i16]);
    }

    #[test]
    fn interleaves_the_channels() {
        let pcm = decode(&file(2, 2, &[frame(1, &[1, 1]), frame(1, &[0xf, 0xf])])).unwrap();
        assert_eq!(pcm.channels, 2);
        assert_eq!(pcm.samples[0], 1);
        assert_eq!(pcm.samples[1], -1);
    }

    #[test]
    fn rejects_bad_files() {
        assert_eq!(decode(b"RIFF"), Err(Error::BadHeader("magic")));
        let mut encrypted = file(1, 1, &[frame(0, &[1])]);
        encrypted[0x13] = 8;
        assert_eq!(decode(&encrypted), Err(Error::Unsupported("encryption")));
        // The header gives 64 samples, but the data has only one frame.
        assert!(decode(&file(1, 64, &[frame(0, &[1])])).is_err());
    }
}
