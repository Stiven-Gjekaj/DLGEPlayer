//! Decodes Nintendo BNTX textures to RGBA pixels.
//!
//! A BNTX file has a header, an `NX  ` section, one `BRTI` block for each
//! texture, and a `BRTD` block with the pixel data. All numbers are
//! little-endian.
//!
//! The `NX  ` section starts at 0x20:
//!
//! - 0x24: the number of textures.
//! - 0x28: the offset of an array of u64 offsets, one for each `BRTI` block.
//!
//! A `BRTI` block:
//!
//! - 0x12: the tile mode. 0 is the block-linear layout of the Tegra X1.
//! - 0x16: the number of mip levels.
//! - 0x1c: the format. The high byte is the type, the low byte the variant.
//! - 0x24, 0x28, 0x2c: the width, the height, and the depth.
//! - 0x30: the number of array layers.
//! - 0x34: the low 3 bits are the log2 of the block height in GOBs.
//! - 0x50: the size of the pixel data.
//! - 0x60: the offset of the name.
//! - 0x70: the offset of an array of u64 offsets, one for each mip level.

use std::fmt;

use tegra_swizzle::BlockHeight;
use tegra_swizzle::surface::{BlockDim, deswizzle_surface};

const MAGIC: &[u8; 4] = b"BNTX";
const NX_MAGIC: &[u8; 4] = b"NX  ";
const BRTI_MAGIC: &[u8; 4] = b"BRTI";
const TILE_BLOCK_LINEAR: u16 = 0;

/// An error from the texture reader.
#[derive(Debug)]
pub enum Error {
    BadMagic(&'static str),
    OutOfBounds(&'static str),
    Unsupported(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::BadMagic(what) => write!(f, "bad magic number in {what}"),
            Error::OutOfBounds(what) => write!(f, "{what} points outside the data"),
            Error::Unsupported(what) => write!(f, "unsupported texture: {what}"),
        }
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;

/// The pixel formats that the decoder knows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    R8,
    Rgba8,
    Bc1,
    Bc2,
    Bc3,
    Bc7,
}

impl Format {
    /// Gives the format for the type byte of a format code.
    pub fn from_code(code: u32) -> Option<Format> {
        Some(match code >> 8 {
            0x02 => Format::R8,
            0x0b => Format::Rgba8,
            0x1a => Format::Bc1,
            0x1b => Format::Bc2,
            0x1c => Format::Bc3,
            0x20 => Format::Bc7,
            _ => return None,
        })
    }

    fn is_block_compressed(self) -> bool {
        !matches!(self, Format::R8 | Format::Rgba8)
    }

    /// Gives the bytes of one pixel, or of one 4x4 block.
    fn bytes_per_unit(self) -> u32 {
        match self {
            Format::R8 => 1,
            Format::Rgba8 => 4,
            Format::Bc1 => 8,
            Format::Bc2 | Format::Bc3 | Format::Bc7 => 16,
        }
    }
}

/// One texture of a BNTX file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Texture {
    pub name: String,
    /// The format code. The low byte is the variant, for example 6 for sRGB.
    pub format_code: u32,
    pub width: u32,
    pub height: u32,
    pub depth: u32,
    pub mip_count: u32,
    pub layer_count: u32,
    pub tile_mode: u16,
    /// The block height in GOBs.
    pub block_height: u32,
    /// The pixel data of all mip levels and layers, in the tiled layout.
    pub data: Vec<u8>,
}

impl Texture {
    pub fn format(&self) -> Option<Format> {
        Format::from_code(self.format_code)
    }

    /// Decodes the first mip level of the first layer to RGBA pixels,
    /// 4 bytes for each pixel, row by row.
    pub fn to_rgba8(&self) -> Result<Vec<u8>> {
        let format = self
            .format()
            .ok_or_else(|| Error::Unsupported(format!("format {:#x}", self.format_code)))?;
        if self.tile_mode != TILE_BLOCK_LINEAR {
            return Err(Error::Unsupported(format!("tile mode {}", self.tile_mode)));
        }
        let block_height = BlockHeight::new(self.block_height)
            .ok_or_else(|| Error::Unsupported(format!("block height {}", self.block_height)))?;
        let block_dim = if format.is_block_compressed() {
            BlockDim::block_4x4()
        } else {
            BlockDim::uncompressed()
        };
        let linear = deswizzle_surface(
            self.width,
            self.height,
            self.depth.max(1),
            &self.data,
            block_dim,
            Some(block_height),
            format.bytes_per_unit(),
            self.mip_count.max(1),
            self.layer_count.max(1),
        )
        .map_err(|e| Error::Unsupported(format!("deswizzle: {e:?}")))?;
        decode(format, self.width, self.height, &linear)
    }
}

/// Decodes linear pixel data of the first mip level to RGBA pixels.
pub fn decode(format: Format, width: u32, height: u32, linear: &[u8]) -> Result<Vec<u8>> {
    let (w, h) = (width as usize, height as usize);
    let mut out = vec![0u8; w * h * 4];
    match format {
        Format::R8 => {
            let src = linear
                .get(..w * h)
                .ok_or(Error::OutOfBounds("pixel data"))?;
            for (px, &v) in out.as_chunks_mut::<4>().0.iter_mut().zip(src) {
                *px = [v, v, v, 255];
            }
        }
        Format::Rgba8 => {
            let src = linear
                .get(..w * h * 4)
                .ok_or(Error::OutOfBounds("pixel data"))?;
            out.copy_from_slice(src);
        }
        _ => {
            let (bw, bh) = (w.div_ceil(4), h.div_ceil(4));
            let unit = format.bytes_per_unit() as usize;
            let src = linear
                .get(..bw * bh * unit)
                .ok_or(Error::OutOfBounds("pixel data"))?;
            // Decode into a buffer of whole blocks, then cut it to the size.
            let pitch = bw * 4 * 4;
            let mut full = vec![0u8; pitch * bh * 4];
            for by in 0..bh {
                for bx in 0..bw {
                    let block = &src[(by * bw + bx) * unit..][..unit];
                    let dst = &mut full[by * 4 * pitch + bx * 16..];
                    match format {
                        Format::Bc1 => bcdec_rs::bc1(block, dst, pitch),
                        Format::Bc2 => bcdec_rs::bc2(block, dst, pitch),
                        Format::Bc3 => bcdec_rs::bc3(block, dst, pitch),
                        _ => bcdec_rs::bc7(block, dst, pitch),
                    }
                }
            }
            for y in 0..h {
                out[y * w * 4..][..w * 4].copy_from_slice(&full[y * pitch..][..w * 4]);
            }
        }
    }
    Ok(out)
}

/// Reads all textures of a BNTX file.
pub fn parse(data: &[u8]) -> Result<Vec<Texture>> {
    if data.len() < 0x40 || &data[..4] != MAGIC {
        return Err(Error::BadMagic("BNTX header"));
    }
    if &data[0x20..0x24] != NX_MAGIC {
        return Err(Error::BadMagic("NX section"));
    }
    let count = u32_at(data, 0x24)? as u64;
    let array = u64_at(data, 0x28)?;
    if count > data.len() as u64 / 8 {
        return Err(Error::OutOfBounds("texture count"));
    }
    let mut textures = Vec::new();
    for i in 0..count {
        let at = u64_at(data, array + 8 * i)?;
        let b = bytes(data, at, 0x78, "BRTI block")?;
        if &b[..4] != BRTI_MAGIC {
            return Err(Error::BadMagic("BRTI block"));
        }
        let field = |o: usize| u32::from_le_bytes(b[o..o + 4].try_into().unwrap());
        let size = field(0x50) as u64;
        let mips = u64::from_le_bytes(b[0x70..0x78].try_into().unwrap());
        let start = u64_at(data, mips)?;
        textures.push(Texture {
            name: string_at(data, u64::from_le_bytes(b[0x60..0x68].try_into().unwrap()))?,
            tile_mode: u16::from_le_bytes([b[0x12], b[0x13]]),
            mip_count: u16::from_le_bytes([b[0x16], b[0x17]]) as u32,
            format_code: field(0x1c),
            width: field(0x24),
            height: field(0x28),
            depth: field(0x2c),
            layer_count: field(0x30),
            block_height: 1 << (field(0x34) & 7),
            data: bytes(data, start, size, "pixel data")?.to_vec(),
        });
    }
    Ok(textures)
}

fn bytes<'a>(data: &'a [u8], at: u64, len: u64, what: &'static str) -> Result<&'a [u8]> {
    let start = usize::try_from(at).map_err(|_| Error::OutOfBounds(what))?;
    let len = usize::try_from(len).map_err(|_| Error::OutOfBounds(what))?;
    data.get(start..start.checked_add(len).ok_or(Error::OutOfBounds(what))?)
        .ok_or(Error::OutOfBounds(what))
}

fn u32_at(data: &[u8], at: u64) -> Result<u32> {
    Ok(u32::from_le_bytes(
        bytes(data, at, 4, "header field")?.try_into().unwrap(),
    ))
}

fn u64_at(data: &[u8], at: u64) -> Result<u64> {
    Ok(u64::from_le_bytes(
        bytes(data, at, 8, "header field")?.try_into().unwrap(),
    ))
}

fn string_at(data: &[u8], at: u64) -> Result<String> {
    let len = u16::from_le_bytes(bytes(data, at, 2, "name")?.try_into().unwrap());
    Ok(String::from_utf8_lossy(bytes(data, at + 2, len as u64, "name")?).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tegra_swizzle::surface::swizzle_surface;

    /// Builds a BNTX file with one texture. The pixel data is linear and
    /// the builder tiles it.
    fn build(format_code: u32, width: u32, height: u32, linear: &[u8]) -> Vec<u8> {
        let format = Format::from_code(format_code).unwrap();
        let block_dim = if format.is_block_compressed() {
            BlockDim::block_4x4()
        } else {
            BlockDim::uncompressed()
        };
        let block_height = 1u32;
        let tiled = swizzle_surface(
            width,
            height,
            1,
            linear,
            block_dim,
            BlockHeight::new(block_height),
            format.bytes_per_unit(),
            1,
            1,
        )
        .unwrap();

        let mut out = vec![0u8; 0x40];
        out[..4].copy_from_slice(MAGIC);
        out[0x20..0x24].copy_from_slice(NX_MAGIC);
        out[0x24..0x28].copy_from_slice(&1u32.to_le_bytes());
        let name = out.len() as u64;
        out.extend(4u16.to_le_bytes());
        out.extend(b"test");
        out.resize(0x60, 0);
        let array = out.len() as u64;
        out[0x28..0x30].copy_from_slice(&array.to_le_bytes());
        let brti = array + 8;
        out.extend(brti.to_le_bytes());
        let mut b = vec![0u8; 0x78];
        b[..4].copy_from_slice(BRTI_MAGIC);
        b[0x16..0x18].copy_from_slice(&1u16.to_le_bytes());
        b[0x1c..0x20].copy_from_slice(&format_code.to_le_bytes());
        b[0x24..0x28].copy_from_slice(&width.to_le_bytes());
        b[0x28..0x2c].copy_from_slice(&height.to_le_bytes());
        b[0x2c..0x30].copy_from_slice(&1u32.to_le_bytes());
        b[0x30..0x34].copy_from_slice(&1u32.to_le_bytes());
        b[0x34..0x38].copy_from_slice(&block_height.trailing_zeros().to_le_bytes());
        b[0x50..0x54].copy_from_slice(&(tiled.len() as u32).to_le_bytes());
        b[0x60..0x68].copy_from_slice(&name.to_le_bytes());
        let mips = brti + 0x78;
        b[0x70..0x78].copy_from_slice(&mips.to_le_bytes());
        out.extend(b);
        let pixels = mips + 8;
        out.extend(pixels.to_le_bytes());
        out.extend(tiled);
        out
    }

    #[test]
    fn decodes_tiled_rgba8() {
        let (w, h) = (70u32, 33u32);
        let linear: Vec<u8> = (0..w * h * 4).map(|i| (i * 7 % 251) as u8).collect();
        let file = build(0x0b01, w, h, &linear);
        let textures = parse(&file).unwrap();
        assert_eq!(textures.len(), 1);
        let t = &textures[0];
        assert_eq!((t.name.as_str(), t.width, t.height), ("test", w, h));
        assert_eq!(t.format(), Some(Format::Rgba8));
        // The tiled data differs from the linear data, so the test proves
        // that the decoder undoes the tiling.
        assert_ne!(&t.data[..linear.len()], &linear[..]);
        assert_eq!(t.to_rgba8().unwrap(), linear);
    }

    #[test]
    fn decodes_bc1_blocks() {
        // Two blocks of 4x4 pixels. Both colours of a block are the same,
        // so each block has one colour: pure red, then pure blue.
        let red = [0x00, 0xf8, 0x00, 0xf8, 0, 0, 0, 0];
        let blue = [0x1f, 0x00, 0x1f, 0x00, 0, 0, 0, 0];
        let linear: Vec<u8> = red.iter().chain(&blue).copied().collect();
        let file = build(0x1a01, 8, 4, &linear);
        let rgba = parse(&file).unwrap()[0].to_rgba8().unwrap();
        assert_eq!(rgba.len(), 8 * 4 * 4);
        assert_eq!(&rgba[..4], &[255, 0, 0, 255]);
        assert_eq!(&rgba[4 * 4..4 * 4 + 4], &[0, 0, 255, 255]);
        assert_eq!(&rgba[(3 * 8 + 7) * 4..][..4], &[0, 0, 255, 255]);
    }

    #[test]
    fn decodes_r8_as_grey() {
        let linear: Vec<u8> = (0..16u8).collect();
        let file = build(0x0201, 4, 4, &linear);
        let rgba = parse(&file).unwrap()[0].to_rgba8().unwrap();
        assert_eq!(&rgba[5 * 4..5 * 4 + 4], &[5, 5, 5, 255]);
    }

    #[test]
    fn rejects_bad_input() {
        assert!(matches!(parse(&[0u8; 0x40]), Err(Error::BadMagic(_))));
        let file = build(0x0b01, 4, 4, &[0u8; 64]);
        for len in [0x30, 0x70, file.len() - 1] {
            assert!(parse(&file[..len]).is_err(), "length {len}");
        }
        let mut t = parse(&file).unwrap().remove(0);
        t.format_code = 0x1505;
        assert!(matches!(t.to_rgba8(), Err(Error::Unsupported(_))));
    }
}
