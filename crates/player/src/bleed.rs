//! Gives colour to the transparent pixels at the edge of an image.
//!
//! A transparent pixel of a sprite sheet is black. When the screen scales a
//! sprite, the filter mixes each pixel with the pixels next to it, so black
//! comes into the edge of the body and of each patch, and a dark line
//! shows. The filter reads only the next pixel, so one ring is enough.

/// Gives each fully transparent pixel next to a visible pixel the mean
/// colour of its visible neighbours. The alpha stays 0. `rgba` holds the
/// pixels of an image of `width` by `height`.
pub fn bleed(rgba: &mut [u8], width: usize, height: usize) {
    if rgba.len() != width * height * 4 {
        return;
    }
    let source = rgba.to_vec();
    for y in 0..height {
        for x in 0..width {
            let at = (y * width + x) * 4;
            if source[at + 3] != 0 {
                continue;
            }
            let mut sum = [0u32; 3];
            let mut n = 0;
            for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1), (-1, -1), (1, -1), (-1, 1), (1, 1)] {
                let (nx, ny) = (x as isize + dx, y as isize + dy);
                if nx < 0 || ny < 0 || nx >= width as isize || ny >= height as isize {
                    continue;
                }
                let n_at = (ny as usize * width + nx as usize) * 4;
                if source[n_at + 3] == 0 {
                    continue;
                }
                for c in 0..3 {
                    sum[c] += source[n_at + c] as u32;
                }
                n += 1;
            }
            if n > 0 {
                for c in 0..3 {
                    rgba[at + c] = (sum[c] / n) as u8;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colours_the_ring_around_visible_pixels() {
        // A 3x1 image: a red pixel, then two transparent black pixels.
        let mut img = vec![255, 0, 0, 255, 0, 0, 0, 0, 0, 0, 0, 0];
        bleed(&mut img, 3, 1);
        assert_eq!(&img[4..8], &[255, 0, 0, 0]);
        // A pixel two steps away keeps its colour.
        assert_eq!(&img[8..12], &[0, 0, 0, 0]);
        // A visible pixel does not change.
        assert_eq!(&img[0..4], &[255, 0, 0, 255]);
    }

    #[test]
    fn mixes_the_colours_of_two_neighbours() {
        let mut img = vec![200, 0, 0, 255, 0, 0, 0, 0, 0, 100, 0, 255];
        bleed(&mut img, 3, 1);
        assert_eq!(&img[4..8], &[100, 50, 0, 0]);
    }
}
