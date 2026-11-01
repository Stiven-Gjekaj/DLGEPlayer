//! Reads `RAW_` images and writes RGBA pixels as PNG or WebP images.
//!
//! A `RAW_` file has the magic `RAW_`, the width and the height as
//! little-endian u16, and then RGBA pixels, 4 bytes for each pixel, row by
//! row. The game uses it for the copyright line.

/// The magic number of a `RAW_` image.
pub const RAW_MAGIC: &[u8; 4] = b"RAW_";

/// Reads a `RAW_` image. Gives the width, the height, and the pixels.
pub fn decode_raw(data: &[u8]) -> Option<(u32, u32, &[u8])> {
    if data.len() < 8 || &data[..4] != RAW_MAGIC {
        return None;
    }
    let width = u16::from_le_bytes([data[4], data[5]]) as u32;
    let height = u16::from_le_bytes([data[6], data[7]]) as u32;
    let pixels = data.get(8..8 + (width * height * 4) as usize)?;
    Some((width, height, pixels))
}

/// Encodes RGBA pixels, 4 bytes for each pixel, row by row, as a PNG file.
pub fn encode_png(width: u32, height: u32, rgba: &[u8]) -> Result<Vec<u8>, png::EncodingError> {
    let mut out = Vec::new();
    let mut encoder = png::Encoder::new(&mut out, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(rgba)?;
    Ok(out)
}

/// The format of the images that the import writes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Format {
    Png,
    /// Lossy WebP with a quality from 0 to 100.
    Webp(f32),
}

impl Format {
    pub fn extension(self) -> &'static str {
        match self {
            Format::Png => "png",
            Format::Webp(_) => "webp",
        }
    }

    /// Encodes RGBA pixels in this format.
    pub fn encode(
        self,
        width: u32,
        height: u32,
        rgba: &[u8],
    ) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
        match self {
            Format::Png => Ok(encode_png(width, height, rgba)?),
            Format::Webp(quality) => Ok(encode_webp(width, height, rgba, quality)?),
        }
    }
}

/// An error from the WebP encoder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebpError(pub &'static str);

impl std::fmt::Display for WebpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "WebP: {}", self.0)
    }
}

impl std::error::Error for WebpError {}

/// Encodes RGBA pixels as a lossy WebP file with libwebp.
pub fn encode_webp(
    width: u32,
    height: u32,
    rgba: &[u8],
    quality: f32,
) -> Result<Vec<u8>, WebpError> {
    if rgba.len() != width as usize * height as usize * 4 {
        return Err(WebpError("the pixel data has the wrong size"));
    }
    if width == 0 || height == 0 || width > 16383 || height > 16383 {
        return Err(WebpError("the size is outside 1 to 16383"));
    }
    let mut out: *mut u8 = std::ptr::null_mut();
    // SAFETY: `rgba` holds `width * height * 4` bytes, which the check
    // above makes sure of. libwebp writes a new buffer to `out`, and
    // `WebPFree` releases it after the copy.
    unsafe {
        let len = libwebp_sys::WebPEncodeRGBA(
            rgba.as_ptr(),
            width as i32,
            height as i32,
            width as i32 * 4,
            quality,
            &mut out,
        );
        if len == 0 || out.is_null() {
            return Err(WebpError("the encoder failed"));
        }
        let data = std::slice::from_raw_parts(out, len).to_vec();
        libwebp_sys::WebPFree(out.cast());
        Ok(data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_pixels() {
        let rgba: Vec<u8> = (0..2 * 3 * 4).map(|i| i as u8).collect();
        let png = encode_png(2, 3, &rgba).unwrap();
        let mut reader = png::Decoder::new(std::io::Cursor::new(png))
            .read_info()
            .unwrap();
        let mut buf = vec![0; reader.output_buffer_size().unwrap()];
        let info = reader.next_frame(&mut buf).unwrap();
        assert_eq!((info.width, info.height), (2, 3));
        assert_eq!(info.color_type, png::ColorType::Rgba);
        assert_eq!(&buf[..info.buffer_size()], &rgba[..]);
    }

    #[test]
    fn reads_raw_images() {
        let mut data = RAW_MAGIC.to_vec();
        data.extend(2u16.to_le_bytes());
        data.extend(1u16.to_le_bytes());
        data.extend([1, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(
            decode_raw(&data),
            Some((2, 1, &[1, 2, 3, 4, 5, 6, 7, 8][..]))
        );
        assert_eq!(decode_raw(&data[..12]), None);
        assert_eq!(decode_raw(b"BNTX\0\0\0\0"), None);
    }

    #[test]
    fn encodes_webp() {
        let rgba: Vec<u8> = (0..64 * 32 * 4).map(|i| (i % 251) as u8).collect();
        let webp = encode_webp(64, 32, &rgba, 90.0).unwrap();
        let decoder = image_webp::WebPDecoder::new(std::io::Cursor::new(&webp)).unwrap();
        assert_eq!(decoder.dimensions(), (64, 32));
        assert!(decoder.has_alpha());
        assert_eq!(Format::Webp(90.0).extension(), "webp");
        assert_eq!(
            Format::Png.encode(64, 32, &rgba).unwrap()[..4],
            [0x89, b'P', b'N', b'G']
        );
    }

    #[test]
    fn refuses_the_wrong_size() {
        assert!(encode_png(2, 2, &[0; 4]).is_err());
        assert!(encode_webp(2, 2, &[0; 4], 90.0).is_err());
        assert!(encode_webp(0, 0, &[], 90.0).is_err());
    }
}
