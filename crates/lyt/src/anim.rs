//! Reads NintendoWare `.bflan` animations, version 8.
//!
//! The file starts with `FLAN` and the same header as a layout. It has two
//! sections:
//!
//! - `pat1`: the tag. The name offset is at 0x0c and the offset of the
//!   group names is at 0x10, both from the start of the section. The group
//!   count is the u16 at 0x0a. Each group name has 36 bytes.
//! - `pai1`: the animation. The frame count is the u16 at 0x08 and the
//!   loop flag is the byte at 0x0a. The texture count is the u16 at 0x0c,
//!   the target count is the u16 at 0x0e, and the offset of the target
//!   offsets is at 0x10.
//!
//! A target is a pane or a material: a 28-byte name, the tag count, the
//! kind (1 for a material), 2 bytes of padding, and the tag offsets from the
//! start of the target. A tag has a magic number, for example `FLPA` for
//! the pane position, a curve count, and the curve offsets from the start
//! of the tag. A curve has the index, the animated value, the curve type,
//! a padding byte, the key count, 2 bytes of padding, and the offset of
//! the keys from the start of the curve. A Hermite key (type 2) has a
//! frame, a value, and a slope as f32. A step key (type 1) has a frame as
//! f32 and a value as u16, then 2 bytes of padding.

use serde::Serialize;

use crate::{Error, Reader, Result};

pub const MAGIC: &[u8; 4] = b"FLAN";

const TARGET_NAME_LEN: usize = 0x1c;
const GROUP_NAME_LEN: usize = 36;
const STEP: u8 = 1;
const HERMITE: u8 = 2;

/// One key of a curve.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Key {
    pub frame: f32,
    pub value: f32,
    /// The slope of a Hermite key. A step key has no slope.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slope: Option<f32>,
}

/// The keys of one animated value.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Curve {
    /// The index, for example of the texture of a material.
    pub index: u8,
    /// The animated value. Its meaning depends on the tag.
    pub target: u8,
    /// True for step keys, false for Hermite keys.
    pub step: bool,
    pub keys: Vec<Key>,
}

/// The curves of one kind, for example the pane position.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Tag {
    /// The magic number, for example `FLPA`.
    pub kind: String,
    pub curves: Vec<Curve>,
}

/// The animation of one pane or material.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Target {
    pub name: String,
    pub material: bool,
    pub tags: Vec<Tag>,
}

/// A parsed animation.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Animation {
    pub name: String,
    pub groups: Vec<String>,
    pub frames: u16,
    pub looping: bool,
    pub textures: Vec<String>,
    pub targets: Vec<Target>,
}

/// Reads an animation.
pub fn parse(data: &[u8]) -> Result<Animation> {
    let r = Reader { data };
    if data.len() < 0x14 || &data[..4] != MAGIC {
        return Err(Error::BadMagic("FLAN header"));
    }
    let mut at = r.u16(6)? as usize;
    let count = r.u16(0x10)? as usize;
    let mut animation = Animation {
        name: String::new(),
        groups: Vec::new(),
        frames: 0,
        looping: false,
        textures: Vec::new(),
        targets: Vec::new(),
    };
    for _ in 0..count {
        let size = r.u32(at + 4)? as usize;
        if size < 8 {
            return Err(Error::OutOfBounds("section size"));
        }
        match r.bytes(at, 4, "section")? {
            b"pat1" => {
                animation.name = r.string(at + r.u32(at + 0x0c)? as usize, usize::MAX)?;
                let groups = at + r.u32(at + 0x10)? as usize;
                animation.groups = (0..r.u16(at + 0x0a)? as usize)
                    .map(|i| r.string(groups + GROUP_NAME_LEN * i, GROUP_NAME_LEN))
                    .collect::<Result<_>>()?;
            }
            b"pai1" => {
                animation.frames = r.u16(at + 8)?;
                animation.looping = r.u8(at + 0x0a)? != 0;
                let textures = r.u16(at + 0x0c)? as usize;
                let names = at + 0x14;
                animation.textures = (0..textures)
                    .map(|i| r.string(names + r.u32(names + 4 * i)? as usize, usize::MAX))
                    .collect::<Result<_>>()?;
                let offsets = at + r.u32(at + 0x10)? as usize;
                animation.targets = (0..r.u16(at + 0x0e)? as usize)
                    .map(|i| target(&r, at + r.u32(offsets + 4 * i)? as usize))
                    .collect::<Result<_>>()?;
            }
            _ => {}
        }
        at += size;
    }
    Ok(animation)
}

fn target(r: &Reader<'_>, at: usize) -> Result<Target> {
    let tags = r.u8(at + TARGET_NAME_LEN)? as usize;
    Ok(Target {
        name: r.string(at, TARGET_NAME_LEN)?,
        material: r.u8(at + TARGET_NAME_LEN + 1)? == 1,
        tags: (0..tags)
            .map(|i| tag(r, at + r.u32(at + 0x20 + 4 * i)? as usize))
            .collect::<Result<_>>()?,
    })
}

fn tag(r: &Reader<'_>, at: usize) -> Result<Tag> {
    let kind = String::from_utf8_lossy(r.bytes(at, 4, "tag")?).into_owned();
    let curves = r.u8(at + 4)? as usize;
    Ok(Tag {
        kind,
        curves: (0..curves)
            .map(|i| curve(r, at + r.u32(at + 8 + 4 * i)? as usize))
            .collect::<Result<_>>()?,
    })
}

fn curve(r: &Reader<'_>, at: usize) -> Result<Curve> {
    let kind = r.u8(at + 2)?;
    let count = r.u16(at + 4)? as usize;
    let keys = at + r.u32(at + 8)? as usize;
    let keys = match kind {
        HERMITE => (0..count)
            .map(|i| {
                let [frame, value, slope] = r.floats(keys + 12 * i)?;
                Ok(Key {
                    frame,
                    value,
                    slope: Some(slope),
                })
            })
            .collect::<Result<_>>()?,
        STEP => (0..count)
            .map(|i| {
                Ok(Key {
                    frame: r.f32(keys + 8 * i)?,
                    value: r.u16(keys + 8 * i + 4)? as f32,
                    slope: None,
                })
            })
            .collect::<Result<_>>()?,
        _ => return Err(Error::OutOfBounds("curve type")),
    };
    Ok(Curve {
        index: r.u8(at)?,
        target: r.u8(at + 1)?,
        step: kind == STEP,
        keys,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn name(text: &str, len: usize) -> Vec<u8> {
        let mut out = text.as_bytes().to_vec();
        out.resize(len, 0);
        out
    }

    /// Builds a curve: the head, then the keys.
    fn curve(target: u8, kind: u8, keys: &[u8], count: u16) -> Vec<u8> {
        let mut out = vec![0, target, kind, 0];
        out.extend(count.to_le_bytes());
        out.extend([0, 0]);
        out.extend(12u32.to_le_bytes());
        out.extend(keys);
        out
    }

    /// Builds a target with one tag that holds the curves.
    fn target(target_name: &str, tag: &[u8; 4], curves: &[Vec<u8>]) -> Vec<u8> {
        let mut out = name(target_name, TARGET_NAME_LEN);
        out.extend([1, 0, 0, 0]);
        out.extend(0x24u32.to_le_bytes());
        out.extend(tag);
        out.extend([curves.len() as u8, 0, 0, 0]);
        let mut at = 8 + 4 * curves.len();
        for c in curves {
            out.extend((at as u32).to_le_bytes());
            at += c.len();
        }
        for c in curves {
            out.extend(c);
        }
        out
    }

    fn animation() -> Vec<u8> {
        let mut pat1 = b"pat1".to_vec();
        let mut body = vec![0, 0, 1, 0];
        body.extend(0x18u32.to_le_bytes());
        body.extend(0x20u32.to_le_bytes());
        body.extend([0; 4]);
        body.extend(name("start", 8));
        body.extend(name("all", GROUP_NAME_LEN));
        pat1.extend(((body.len() + 8) as u32).to_le_bytes());
        pat1.extend(body);

        let hermite: Vec<u8> = [0.0f32, 0.0, 0.0, 10.0, 1.0, 0.5]
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .collect();
        let mut step = 5f32.to_le_bytes().to_vec();
        step.extend([1, 0, 0, 0]);
        let t = target(
            "title",
            b"FLPA",
            &[curve(7, HERMITE, &hermite, 2), curve(0, STEP, &step, 1)],
        );
        let mut pai1 = b"pai1".to_vec();
        let mut body = 80u16.to_le_bytes().to_vec();
        body.extend([1, 0]);
        body.extend(0u16.to_le_bytes());
        body.extend(1u16.to_le_bytes());
        body.extend(0x14u32.to_le_bytes());
        body.extend(0x18u32.to_le_bytes());
        body.extend(t);
        pai1.extend(((body.len() + 8) as u32).to_le_bytes());
        pai1.extend(body);

        let mut out = MAGIC.to_vec();
        out.extend([0xff, 0xfe]);
        out.extend(0x14u16.to_le_bytes());
        out.extend(0x0806_0000u32.to_le_bytes());
        out.extend(((0x14 + pat1.len() + pai1.len()) as u32).to_le_bytes());
        out.extend(2u16.to_le_bytes());
        out.extend([0, 0]);
        out.extend(pat1);
        out.extend(pai1);
        out
    }

    #[test]
    fn reads_an_animation() {
        let a = parse(&animation()).unwrap();
        assert_eq!(a.name, "start");
        assert_eq!(a.groups, ["all"]);
        assert_eq!(a.frames, 80);
        assert!(a.looping);
        assert_eq!(a.targets.len(), 1);
        let t = &a.targets[0];
        assert_eq!(t.name, "title");
        assert!(!t.material);
        assert_eq!(t.tags[0].kind, "FLPA");
        let curves = &t.tags[0].curves;
        assert_eq!(curves[0].target, 7);
        assert!(!curves[0].step);
        assert_eq!(
            curves[0].keys[1],
            Key {
                frame: 10.0,
                value: 1.0,
                slope: Some(0.5)
            }
        );
        assert!(curves[1].step);
        assert_eq!(
            curves[1].keys,
            [Key {
                frame: 5.0,
                value: 1.0,
                slope: None
            }]
        );
    }

    #[test]
    fn rejects_bad_animations() {
        assert!(matches!(parse(b"FLYT"), Err(Error::BadMagic(_))));
        let bytes = animation();
        assert!(parse(&bytes[..bytes.len() - 4]).is_err());
    }
}
