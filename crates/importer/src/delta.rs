//! Makes and applies a binary delta from an old file to a new file.
//!
//! The English patch ships as deltas. A delta copies the parts of the new
//! file that are in the old file of the user, and holds only the other
//! bytes: the translation and the changes to the structure.
//!
//! The format: the magic `DLD1`, the size of the new file (u64), the hash
//! of the old file (u64, FNV-1a), then the operations. An operation is a
//! tag byte and varints: tag 0 copies `length` bytes from `offset` in the
//! old file, and tag 1 inserts the next `length` bytes of the delta.

use std::collections::HashMap;
use std::fmt;

const MAGIC: &[u8; 4] = b"DLD1";
const COPY: u8 = 0;
const INSERT: u8 = 1;
/// The length of the windows that the index finds.
const WINDOW: usize = 16;
/// The distance between two windows of the old file in the index.
const STEP: usize = 4;
/// The number of old positions that the index keeps for one window.
const CANDIDATES: usize = 8;
/// A copy shorter than this costs more than an insert.
const MIN_COPY: usize = 24;

/// An error from `apply`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    BadDelta(&'static str),
    /// The old file is not the file that the delta was made from.
    WrongSource,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::BadDelta(why) => write!(f, "bad delta: {why}"),
            Error::WrongSource => write!(
                f,
                "the file of the dump is not the file that the patch expects"
            ),
        }
    }
}

impl std::error::Error for Error {}

/// Gives the FNV-1a hash of some bytes.
pub fn hash(data: &[u8]) -> u64 {
    data.iter().fold(0xcbf2_9ce4_8422_2325, |h, &b| {
        (h ^ b as u64).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

fn put_varint(out: &mut Vec<u8>, mut v: u64) {
    while v >= 0x80 {
        out.push(v as u8 | 0x80);
        v >>= 7;
    }
    out.push(v as u8);
}

fn get_varint(data: &[u8], at: &mut usize) -> Result<u64, Error> {
    let mut v = 0u64;
    for shift in (0..64).step_by(7) {
        let b = *data.get(*at).ok_or(Error::BadDelta("cut varint"))?;
        *at += 1;
        v |= ((b & 0x7f) as u64) << shift;
        if b < 0x80 {
            return Ok(v);
        }
    }
    Err(Error::BadDelta("long varint"))
}

fn window(data: &[u8], at: usize) -> u128 {
    u128::from_le_bytes(data[at..at + WINDOW].try_into().unwrap())
}

/// Makes a delta from `old` to `new`.
pub fn diff(old: &[u8], new: &[u8]) -> Vec<u8> {
    let mut index: HashMap<u128, Vec<u32>> = HashMap::new();
    if old.len() >= WINDOW {
        for at in (0..=old.len() - WINDOW).step_by(STEP) {
            let list = index.entry(window(old, at)).or_default();
            if list.len() < CANDIDATES {
                list.push(at as u32);
            }
        }
    }
    let mut out = MAGIC.to_vec();
    out.extend((new.len() as u64).to_le_bytes());
    out.extend(hash(old).to_le_bytes());
    let mut pending = 0; // The start of the bytes that wait for an insert.
    let mut i = 0;
    while i + WINDOW <= new.len() {
        let best = index.get(&window(new, i)).and_then(|list| {
            list.iter()
                .map(|&p| {
                    let p = p as usize;
                    let len = old[p..]
                        .iter()
                        .zip(&new[i..])
                        .take_while(|(a, b)| a == b)
                        .count();
                    (p, len)
                })
                .max_by_key(|&(_, len)| len)
        });
        let Some((mut from, mut len)) = best.filter(|&(_, len)| len >= MIN_COPY) else {
            i += 1;
            continue;
        };
        // Extend the copy back into the bytes that wait.
        let mut start = i;
        while start > pending && from > 0 && old[from - 1] == new[start - 1] {
            start -= 1;
            from -= 1;
            len += 1;
        }
        if start > pending {
            out.push(INSERT);
            put_varint(&mut out, (start - pending) as u64);
            out.extend(&new[pending..start]);
        }
        out.push(COPY);
        put_varint(&mut out, from as u64);
        put_varint(&mut out, len as u64);
        i = start + len;
        pending = i;
    }
    if pending < new.len() {
        out.push(INSERT);
        put_varint(&mut out, (new.len() - pending) as u64);
        out.extend(&new[pending..]);
    }
    out
}

/// Applies a delta to `old`. The hash of `old` must agree with the delta.
pub fn apply(old: &[u8], delta: &[u8]) -> Result<Vec<u8>, Error> {
    if delta.len() < 20 || &delta[..4] != MAGIC {
        return Err(Error::BadDelta("magic"));
    }
    let size = u64::from_le_bytes(delta[4..12].try_into().unwrap());
    if u64::from_le_bytes(delta[12..20].try_into().unwrap()) != hash(old) {
        return Err(Error::WrongSource);
    }
    let size = usize::try_from(size).map_err(|_| Error::BadDelta("size"))?;
    let mut out = Vec::with_capacity(size.min(1 << 30));
    let mut at = 20;
    while at < delta.len() {
        let tag = delta[at];
        at += 1;
        match tag {
            COPY => {
                let from = get_varint(delta, &mut at)? as usize;
                let len = get_varint(delta, &mut at)? as usize;
                let part = from
                    .checked_add(len)
                    .and_then(|end| old.get(from..end))
                    .ok_or(Error::BadDelta("copy outside the old file"))?;
                out.extend_from_slice(part);
            }
            INSERT => {
                let len = get_varint(delta, &mut at)? as usize;
                let part = at
                    .checked_add(len)
                    .and_then(|end| delta.get(at..end))
                    .ok_or(Error::BadDelta("cut insert"))?;
                out.extend_from_slice(part);
                at += len;
            }
            _ => return Err(Error::BadDelta("unknown operation")),
        }
        if out.len() > size {
            return Err(Error::BadDelta("output too long"));
        }
    }
    if out.len() != size {
        return Err(Error::BadDelta("output too short"));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Gives bytes from a simple generator, so that the tests repeat.
    fn noise(seed: u64, len: usize) -> Vec<u8> {
        let mut x = seed;
        (0..len)
            .map(|_| {
                x ^= x << 13;
                x ^= x >> 7;
                x ^= x << 17;
                x as u8
            })
            .collect()
    }

    #[test]
    fn round_trips_edits() {
        let old = noise(1, 20_000);
        let mut new = old.clone();
        new.splice(5000..5000, b"an inserted English line".iter().copied());
        new[12_000..12_040].copy_from_slice(&noise(2, 40));
        new.drain(15_000..15_500);
        new.extend(noise(3, 300));
        let delta = diff(&old, &new);
        assert_eq!(apply(&old, &delta).unwrap(), new);
        // The delta holds the changes, not the whole file.
        assert!(delta.len() < 1000, "delta is {} bytes", delta.len());
    }

    #[test]
    fn round_trips_odd_cases() {
        let cases: [(&[u8], &[u8]); 5] = [
            (b"", b""),
            (b"", b"new"),
            (b"old", b""),
            (&[0; 100], &[0; 300]),
            (b"short", b"shorter"),
        ];
        for (old, new) in cases {
            assert_eq!(apply(old, &diff(old, new)).unwrap(), new);
        }
        let old = noise(4, 5000);
        let moved: Vec<u8> = old[2500..].iter().chain(&old[..2500]).copied().collect();
        let delta = diff(&old, &moved);
        assert_eq!(apply(&old, &delta).unwrap(), moved);
        assert!(delta.len() < 100);
    }

    #[test]
    fn refuses_the_wrong_old_file() {
        let delta = diff(b"the file of the dump", b"the patched file");
        assert_eq!(apply(b"another file", &delta), Err(Error::WrongSource));
    }

    #[test]
    fn refuses_bad_deltas() {
        let old = noise(5, 1000);
        let delta = diff(&old, &noise(6, 1000));
        assert_eq!(apply(&old, b"DLD0"), Err(Error::BadDelta("magic")));
        assert!(apply(&old, &delta[..delta.len() - 1]).is_err());
        let mut copy = delta[..20].to_vec();
        copy.push(COPY);
        put_varint(&mut copy, 990);
        put_varint(&mut copy, 20);
        assert_eq!(
            apply(&old, &copy),
            Err(Error::BadDelta("copy outside the old file"))
        );
    }
}
