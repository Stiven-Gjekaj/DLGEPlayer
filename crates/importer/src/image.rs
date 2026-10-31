//! Reads `RAW_` images and writes RGBA pixels as PNG images.
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
    fn refuses_the_wrong_size() {
        assert!(encode_png(2, 2, &[0; 4]).is_err());
    }
}
