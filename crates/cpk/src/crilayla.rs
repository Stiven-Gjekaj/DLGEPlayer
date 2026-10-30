//! Decompresses CRILAYLA data.
//!
//! The layout of a compressed file:
//!
//! - 0x00: the magic number `CRILAYLA`.
//! - 0x08: the size of the output without the prefix, as a little-endian u32.
//! - 0x0c: the size of the compressed stream, as a little-endian u32.
//! - 0x10: the compressed stream.
//! - after the stream: 0x100 bytes of prefix that are not compressed.
//!
//! The decoder reads the stream from its last byte to its first byte.
//! It reads the bits of each byte from the high bit to the low bit.
//! It writes the output from the last byte to the first byte.

use crate::error::{Error, Result};

pub const MAGIC: &[u8; 8] = b"CRILAYLA";
const PREFIX_LEN: usize = 0x100;
const VLE_LENS: [u32; 4] = [2, 3, 5, 8];

/// Tells if the data starts with the CRILAYLA magic number.
pub fn is_compressed(data: &[u8]) -> bool {
    data.len() >= 0x10 && &data[..8] == MAGIC
}

struct BitReader<'a> {
    data: &'a [u8],
    /// The index of the next byte to read. The reader moves down.
    next: usize,
    pool: u8,
    left: u32,
}

impl BitReader<'_> {
    fn bits(&mut self, count: u32) -> Result<u32> {
        let mut out = 0u32;
        let mut produced = 0;
        while produced < count {
            if self.left == 0 {
                self.pool = *self
                    .data
                    .get(self.next)
                    .ok_or(Error::OutOfBounds("crilayla stream"))?;
                self.left = 8;
                self.next = self.next.wrapping_sub(1);
            }
            let take = self.left.min(count - produced);
            out <<= take;
            out |= ((self.pool as u32) >> (self.left - take)) & ((1 << take) - 1);
            self.left -= take;
            produced += take;
        }
        Ok(out)
    }
}

/// Decompresses CRILAYLA data. The output holds the prefix and the body.
pub fn decompress(input: &[u8]) -> Result<Vec<u8>> {
    if !is_compressed(input) {
        return Err(Error::BadMagic("CRILAYLA"));
    }
    let body_len = u32::from_le_bytes(input[8..12].try_into().unwrap()) as usize;
    let stream_len = u32::from_le_bytes(input[12..16].try_into().unwrap()) as usize;
    let prefix_at = 0x10 + stream_len;
    let prefix = input
        .get(prefix_at..prefix_at + PREFIX_LEN)
        .ok_or(Error::OutOfBounds("crilayla prefix"))?;

    let mut out = vec![0u8; PREFIX_LEN + body_len];
    out[..PREFIX_LEN].copy_from_slice(prefix);
    if body_len == 0 {
        return Ok(out);
    }

    let mut bits = BitReader {
        data: &input[..prefix_at],
        next: prefix_at - 1,
        pool: 0,
        left: 0,
    };
    let last = PREFIX_LEN + body_len - 1;
    let mut written = 0usize;
    while written < body_len {
        if bits.bits(1)? == 1 {
            let mut from = last - written + bits.bits(13)? as usize + 3;
            let mut len = 3usize;
            let mut all_full = true;
            for width in VLE_LENS {
                let part = bits.bits(width)?;
                len += part as usize;
                if part != (1 << width) - 1 {
                    all_full = false;
                    break;
                }
            }
            if all_full {
                loop {
                    let part = bits.bits(8)?;
                    len += part as usize;
                    if part != 0xff {
                        break;
                    }
                }
            }
            if len > body_len - written {
                return Err(Error::OutOfBounds("crilayla match length"));
            }
            for _ in 0..len {
                let byte = *out.get(from).ok_or(Error::OutOfBounds("crilayla match"))?;
                out[last - written] = byte;
                from -= 1;
                written += 1;
            }
        } else {
            out[last - written] = bits.bits(8)? as u8;
            written += 1;
        }
    }
    Ok(out)
}

/// Compresses data in the CRILAYLA format.
///
/// The encoder uses a simple greedy search. It exists so that tests can
/// make compressed data. The data must be at least 0x100 bytes long.
pub fn compress(data: &[u8]) -> Result<Vec<u8>> {
    if data.len() < PREFIX_LEN {
        return Err(Error::Unsupported("crilayla input is shorter than 0x100 bytes".into()));
    }
    let body = &data[PREFIX_LEN..];
    let mut bits = BitWriter::default();

    // The decoder makes the body from the end. The encoder also starts at
    // the end. A match copies from higher positions that are already done.
    let mut pos = body.len();
    while pos > 0 {
        let p = pos - 1;
        let (distance, len) = longest_match(body, p);
        if len >= 3 {
            bits.put(1, 1);
            bits.put((distance - 3) as u32, 13);
            put_length(&mut bits, len - 3);
            pos -= len;
        } else {
            bits.put(0, 1);
            bits.put(body[p] as u32, 8);
            pos -= 1;
        }
    }

    let mut stream = bits.finish();
    stream.reverse();

    let mut out = Vec::with_capacity(0x10 + stream.len() + PREFIX_LEN);
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&(body.len() as u32).to_le_bytes());
    out.extend_from_slice(&(stream.len() as u32).to_le_bytes());
    out.extend_from_slice(&stream);
    out.extend_from_slice(&data[..PREFIX_LEN]);
    Ok(out)
}

fn longest_match(body: &[u8], p: usize) -> (usize, usize) {
    const MAX_DISTANCE: usize = (1 << 13) - 1 + 3;
    let mut best = (0, 0);
    for distance in 3..=MAX_DISTANCE {
        if p + distance >= body.len() {
            break;
        }
        let mut len = 0;
        while len <= p && body[p - len] == body[p - len + distance] {
            len += 1;
        }
        if len > best.1 {
            best = (distance, len);
        }
    }
    best
}

fn put_length(bits: &mut BitWriter, mut extra: usize) {
    for width in VLE_LENS {
        let full = (1usize << width) - 1;
        if extra < full {
            bits.put(extra as u32, width);
            return;
        }
        bits.put(full as u32, width);
        extra -= full;
    }
    loop {
        if extra < 0xff {
            bits.put(extra as u32, 8);
            return;
        }
        bits.put(0xff, 8);
        extra -= 0xff;
    }
}

#[derive(Default)]
struct BitWriter {
    bytes: Vec<u8>,
    used: u32,
}

impl BitWriter {
    fn put(&mut self, value: u32, count: u32) {
        for i in (0..count).rev() {
            if self.used == 0 {
                self.bytes.push(0);
            }
            let bit = ((value >> i) & 1) as u8;
            *self.bytes.last_mut().unwrap() |= bit << (7 - self.used);
            self.used = (self.used + 1) % 8;
        }
    }

    fn finish(self) -> Vec<u8> {
        self.bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip(data: &[u8]) {
        let packed = compress(data).unwrap();
        assert!(is_compressed(&packed));
        assert_eq!(decompress(&packed).unwrap(), data);
    }

    #[test]
    fn round_trip_prefix_only() {
        round_trip(&[7u8; PREFIX_LEN]);
    }

    #[test]
    fn round_trip_random_bytes() {
        // A small linear congruential generator gives data with few matches.
        let mut x: u32 = 12345;
        let data: Vec<u8> = (0..2000)
            .map(|_| {
                x = x.wrapping_mul(1103515245).wrapping_add(12345);
                (x >> 16) as u8
            })
            .collect();
        round_trip(&data);
    }

    #[test]
    fn round_trip_long_runs() {
        // Long runs make matches that need the 8-bit length extension.
        let mut data = vec![0u8; PREFIX_LEN];
        data.extend(std::iter::repeat_n(b'a', 1000));
        data.extend(b"STCM2L text text text text");
        data.extend(std::iter::repeat_n(b'b', 300));
        round_trip(&data);
    }

    #[test]
    fn compression_makes_repeated_data_smaller() {
        let mut data = vec![0u8; PREFIX_LEN];
        data.extend(std::iter::repeat_n(b"hello world ".as_slice(), 200).flatten());
        let packed = compress(&data).unwrap();
        assert!(packed.len() < data.len() / 4);
    }

    #[test]
    fn decodes_a_hand_made_stream() {
        // The body is "xyzxyz". The encoder reads from the end:
        // literal 'z', literal 'y', literal 'x', then a match of 3 bytes
        // at distance 3.
        let mut bits = BitWriter::default();
        for byte in [b'z', b'y', b'x'] {
            bits.put(0, 1);
            bits.put(byte as u32, 8);
        }
        bits.put(1, 1);
        bits.put(0, 13);
        bits.put(0, 2);
        let mut stream = bits.finish();
        stream.reverse();

        let mut input = MAGIC.to_vec();
        input.extend_from_slice(&6u32.to_le_bytes());
        input.extend_from_slice(&(stream.len() as u32).to_le_bytes());
        input.extend_from_slice(&stream);
        input.extend_from_slice(&[0u8; PREFIX_LEN]);

        let out = decompress(&input).unwrap();
        assert_eq!(&out[PREFIX_LEN..], b"xyzxyz");
    }

    #[test]
    fn rejects_truncated_input() {
        let mut data = vec![1u8; PREFIX_LEN];
        data.extend(0..=255u8);
        let packed = compress(&data).unwrap();
        assert!(decompress(&packed[..packed.len() - 1]).is_err());
    }
}
