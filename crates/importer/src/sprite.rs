//! Builds character sprites from a sprite sheet and its row in
//! `static/chara.bin`.
//!
//! A sheet holds the body at its top-left corner, and patches for the eyes
//! and the mouth. A row of `chara.bin` has 13 groups of a name and 12
//! integers. The first group describes the sheet:
//!
//! | Index | Value |
//! |---|---|
//! | 0, 1 | the width and the height of the sheet |
//! | 2, 3 | the width and the height of the body |
//! | 4, 5 | the width and the height of an eye patch |
//! | 6, 7 | the position of the eye patch on the body |
//! | 8, 9 | the width and the height of a mouth patch |
//! | 10, 11 | the position of the mouth patch on the body |
//!
//! Each other group is an expression. Its 12 integers are 6 positions in
//! the sheet: 3 eye patches for a blink, then 3 mouth patches for speech.
//! A group with the name `""` and the values -1 is not used.

use std::fmt;

const GROUPS: usize = 13;
const GROUP_LEN: usize = 13;

/// An error from the sprite builder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// The row does not have the shape of a `chara.bin` row.
    BadRow(String),
    /// A patch is outside the sheet.
    OutOfBounds(&'static str),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::BadRow(why) => write!(f, "bad sprite row: {why}"),
            Error::OutOfBounds(what) => write!(f, "the {what} is outside the sheet"),
        }
    }
}

impl std::error::Error for Error {}

/// A position or a size in pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Point {
    pub x: u32,
    pub y: u32,
}

/// One expression: the frames of the eyes and of the mouth.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Expression {
    pub name: String,
    pub eyes: [Point; 3],
    pub mouths: [Point; 3],
}

/// The layout of one sprite sheet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sprite {
    pub name: String,
    pub sheet: Point,
    pub body: Point,
    pub eye_size: Point,
    pub eye_at: Point,
    pub mouth_size: Point,
    pub mouth_at: Point,
    pub expressions: Vec<Expression>,
}

impl Sprite {
    /// Reads a row of `chara.bin`. A cell is a name or an integer.
    pub fn from_row(row: &[bea::table::Cell]) -> Result<Sprite, Error> {
        if row.len() != GROUPS * GROUP_LEN {
            return Err(Error::BadRow(format!("{} cells, not 169", row.len())));
        }
        let group = |g: usize| -> Result<(String, [i16; 12]), Error> {
            let cells = &row[g * GROUP_LEN..(g + 1) * GROUP_LEN];
            let name = cells[0]
                .as_text()
                .ok_or_else(|| Error::BadRow(format!("group {g} has no name")))?;
            let mut values = [0i16; 12];
            for (v, cell) in values.iter_mut().zip(&cells[1..]) {
                *v = cell
                    .as_int()
                    .ok_or_else(|| Error::BadRow(format!("group {g} has a text value")))?;
            }
            Ok((name.to_string(), values))
        };
        let point = |a: i16, b: i16| -> Result<Point, Error> {
            if a < 0 || b < 0 {
                return Err(Error::BadRow(format!("negative value ({a}, {b})")));
            }
            Ok(Point {
                x: a as u32,
                y: b as u32,
            })
        };

        let (name, v) = group(0)?;
        let mut expressions = Vec::new();
        for g in 1..GROUPS {
            let (expr, e) = group(g)?;
            if expr.is_empty() && e.iter().all(|&x| x == -1) {
                continue;
            }
            expressions.push(Expression {
                name: expr,
                eyes: [point(e[0], e[1])?, point(e[2], e[3])?, point(e[4], e[5])?],
                mouths: [point(e[6], e[7])?, point(e[8], e[9])?, point(e[10], e[11])?],
            });
        }
        Ok(Sprite {
            name,
            sheet: point(v[0], v[1])?,
            body: point(v[2], v[3])?,
            eye_size: point(v[4], v[5])?,
            eye_at: point(v[6], v[7])?,
            mouth_size: point(v[8], v[9])?,
            mouth_at: point(v[10], v[11])?,
            expressions,
        })
    }

    /// Builds the body with one expression. `sheet` holds RGBA pixels of
    /// the sheet. The frame selects the blink and the speech frame, 0 to 2.
    /// The result holds RGBA pixels of the size of the body.
    pub fn compose(
        &self,
        sheet: &[u8],
        expression: usize,
        eye_frame: usize,
        mouth_frame: usize,
    ) -> Result<Vec<u8>, Error> {
        let expr = self
            .expressions
            .get(expression)
            .ok_or(Error::OutOfBounds("expression"))?;
        let eye = *expr
            .eyes
            .get(eye_frame)
            .ok_or(Error::OutOfBounds("eye frame"))?;
        let mouth = *expr
            .mouths
            .get(mouth_frame)
            .ok_or(Error::OutOfBounds("mouth frame"))?;
        if sheet.len() != (self.sheet.x * self.sheet.y * 4) as usize {
            return Err(Error::OutOfBounds("sheet size"));
        }
        let mut out = vec![0u8; (self.body.x * self.body.y * 4) as usize];
        let body_origin = Point { x: 0, y: 0 };
        self.copy(
            sheet,
            &mut out,
            body_origin,
            self.body,
            body_origin,
            false,
            "body",
        )?;
        self.copy(
            sheet,
            &mut out,
            eye,
            self.eye_size,
            self.eye_at,
            true,
            "eye patch",
        )?;
        self.copy(
            sheet,
            &mut out,
            mouth,
            self.mouth_size,
            self.mouth_at,
            true,
            "mouth patch",
        )?;
        Ok(out)
    }

    /// Copies a rectangle of the sheet onto the body. With `blend`, the
    /// copy uses the alpha of the patch.
    #[allow(clippy::too_many_arguments)]
    fn copy(
        &self,
        sheet: &[u8],
        out: &mut [u8],
        from: Point,
        size: Point,
        to: Point,
        blend: bool,
        what: &'static str,
    ) -> Result<(), Error> {
        if from.x + size.x > self.sheet.x || from.y + size.y > self.sheet.y {
            return Err(Error::OutOfBounds(what));
        }
        if to.x + size.x > self.body.x || to.y + size.y > self.body.y {
            return Err(Error::OutOfBounds(what));
        }
        for y in 0..size.y {
            for x in 0..size.x {
                let s = (((from.y + y) * self.sheet.x + from.x + x) * 4) as usize;
                let d = (((to.y + y) * self.body.x + to.x + x) * 4) as usize;
                let src = &sheet[s..s + 4];
                if !blend {
                    out[d..d + 4].copy_from_slice(src);
                    continue;
                }
                let a = src[3] as u32;
                for c in 0..3 {
                    let dst = out[d + c] as u32;
                    out[d + c] = ((src[c] as u32 * a + dst * (255 - a)) / 255) as u8;
                }
                out[d + 3] = (a + out[d + 3] as u32 * (255 - a) / 255) as u8;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bea::table::Cell;

    fn group(name: &str, values: [i16; 12]) -> Vec<Cell> {
        let mut g = vec![Cell::Text(name.into())];
        g.extend(values.map(Cell::Int));
        g
    }

    /// A sheet of 8x4 pixels. The body is the left 4x4 pixels. One eye
    /// patch of 2x1 pixels goes to (1, 1). One mouth patch of 1x1 pixel goes
    /// to (2, 3).
    fn row() -> Vec<Cell> {
        let mut r = group("test_A", [8, 4, 4, 4, 2, 1, 1, 1, 1, 1, 2, 3]);
        r.extend(group("emi", [4, 0, 4, 1, 4, 2, 6, 0, 6, 1, 6, 2]));
        for _ in 2..GROUPS {
            r.extend(group("", [-1; 12]));
        }
        r
    }

    fn sheet() -> Vec<u8> {
        let mut s = vec![0u8; 8 * 4 * 4];
        let mut set = |x: usize, y: usize, rgba: [u8; 4]| {
            s[(y * 8 + x) * 4..][..4].copy_from_slice(&rgba);
        };
        for y in 0..4 {
            for x in 0..4 {
                set(x, y, [10, 10, 10, 255]);
            }
        }
        // The eye frames are opaque red, green, and blue.
        set(4, 0, [255, 0, 0, 255]);
        set(5, 0, [255, 0, 0, 255]);
        set(4, 1, [0, 255, 0, 255]);
        set(5, 1, [0, 255, 0, 255]);
        // The second mouth frame is transparent.
        set(6, 0, [200, 200, 200, 255]);
        set(6, 1, [0, 0, 0, 0]);
        s
    }

    fn pixel(img: &[u8], x: usize, y: usize) -> [u8; 4] {
        img[(y * 4 + x) * 4..][..4].try_into().unwrap()
    }

    #[test]
    fn reads_a_row() {
        let sprite = Sprite::from_row(&row()).unwrap();
        assert_eq!(sprite.name, "test_A");
        assert_eq!(sprite.sheet, Point { x: 8, y: 4 });
        assert_eq!(sprite.eye_at, Point { x: 1, y: 1 });
        assert_eq!(sprite.expressions.len(), 1);
        assert_eq!(sprite.expressions[0].name, "emi");
        assert_eq!(sprite.expressions[0].mouths[2], Point { x: 6, y: 2 });
    }

    #[test]
    fn composes_the_body_with_patches() {
        let sprite = Sprite::from_row(&row()).unwrap();
        let img = sprite.compose(&sheet(), 0, 0, 0).unwrap();
        assert_eq!(img.len(), 4 * 4 * 4);
        assert_eq!(pixel(&img, 0, 0), [10, 10, 10, 255]);
        assert_eq!(pixel(&img, 1, 1), [255, 0, 0, 255]);
        assert_eq!(pixel(&img, 2, 1), [255, 0, 0, 255]);
        assert_eq!(pixel(&img, 2, 3), [200, 200, 200, 255]);

        let img = sprite.compose(&sheet(), 0, 1, 1).unwrap();
        assert_eq!(pixel(&img, 1, 1), [0, 255, 0, 255]);
        // A transparent patch keeps the body.
        assert_eq!(pixel(&img, 2, 3), [10, 10, 10, 255]);
    }

    #[test]
    fn rejects_bad_rows_and_frames() {
        assert!(matches!(
            Sprite::from_row(&row()[..20]),
            Err(Error::BadRow(_))
        ));
        let mut r = row();
        r[1] = Cell::Int(-5);
        assert!(matches!(Sprite::from_row(&r), Err(Error::BadRow(_))));
        let sprite = Sprite::from_row(&row()).unwrap();
        assert!(sprite.compose(&sheet(), 1, 0, 0).is_err());
        assert!(sprite.compose(&sheet(), 0, 3, 0).is_err());
        assert!(sprite.compose(&sheet()[..16], 0, 0, 0).is_err());
    }
}
