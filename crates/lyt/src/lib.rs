//! Reads NintendoWare `.bflyt` layouts, version 8. The `anim` module reads
//! the `.bflan` animations.
//!
//! All numbers are little-endian. The file starts with `FLYT`, the byte
//! order mark `ff fe`, the header size, the version, the file size, and the
//! number of sections. Each section has a 4-byte magic number and a u32
//! size that includes the 8-byte section header.
//!
//! | Section | Contents |
//! |---|---|
//! | `lyt1` | the layout size as two f32 at 0x0c, and the name at 0x1c |
//! | `txl1` | the texture names |
//! | `fnl1` | the font names |
//! | `mat1` | the materials |
//! | `pan1`, `pic1`, `txt1`, `wnd1`, `bnd1`, `prt1`, `ali1` | the panes |
//! | `pas1`, `pae1` | the start and the end of the children of a pane |
//! | `grp1`, `grs1`, `gre1` | the groups, and the start and the end of their children |
//! | `usd1`, `cnt1` | user data and the control data. The reader skips them. |
//!
//! A pane section starts with a common block of 0x4c bytes after the
//! section header: the flags, the origin, the alpha, an extra flag byte,
//! the name (24 bytes), the user data (8 bytes), the translation (3 f32),
//! the rotation (3 f32), the scale (2 f32), and the size (2 f32).

use std::fmt;

pub mod anim;

use serde::Serialize;

pub const MAGIC: &[u8; 4] = b"FLYT";

const PANE_LEN: usize = 0x54;
const NAME_LEN: usize = 24;
const MATERIAL_NAME_LEN: usize = 0x1c;
const GROUP_NAME_LEN: usize = 0x22;

/// An error from the layout reader.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    BadMagic(&'static str),
    OutOfBounds(&'static str),
    /// The children of the panes or of the groups do not close.
    BadTree(&'static str),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::BadMagic(what) => write!(f, "bad magic number in {what}"),
            Error::OutOfBounds(what) => write!(f, "{what} points outside the data"),
            Error::BadTree(what) => write!(f, "bad tree of {what}"),
        }
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;

/// One texture of a material.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TexMap {
    /// The index in `Layout::textures`.
    pub texture: u16,
    pub wrap_s: u8,
    pub wrap_t: u8,
}

/// The part of a material that the player uses.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Material {
    pub name: String,
    pub flags: u32,
    /// The black color (the color for value 0), RGBA.
    pub black: [u8; 4],
    /// The white color (the color for value 1), RGBA.
    pub white: [u8; 4],
    pub textures: Vec<TexMap>,
}

/// The data of each kind of pane.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Kind {
    /// A pane that only holds other panes.
    Null,
    Picture {
        material: u16,
        /// The colors of the corners: top left, top right, bottom left,
        /// bottom right. RGBA.
        vertex_colors: [[u8; 4]; 4],
        /// The texture coordinates of the corners, one set for each texture.
        uvs: Vec<[[f32; 2]; 4]>,
    },
    Text {
        material: u16,
        font: u16,
        text: String,
        font_size: [f32; 2],
        char_space: f32,
        line_space: f32,
        /// The origin of the text in the pane.
        position: u8,
        alignment: u8,
        top_color: [u8; 4],
        bottom_color: [u8; 4],
    },
    Window,
    Bounding,
    Alignment,
    /// A pane that shows another layout of the same archive.
    Part {
        layout: String,
        magnify: [f32; 2],
    },
}

/// One pane and its children.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Pane {
    pub name: String,
    #[serde(flatten)]
    pub kind: Kind,
    pub visible: bool,
    pub flags: u8,
    pub origin: u8,
    pub alpha: u8,
    pub translate: [f32; 3],
    pub rotate: [f32; 3],
    pub scale: [f32; 2],
    pub size: [f32; 2],
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<Pane>,
}

/// A named list of panes.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Group {
    pub name: String,
    pub panes: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<Group>,
}

/// A parsed layout.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Layout {
    pub name: String,
    pub width: f32,
    pub height: f32,
    pub textures: Vec<String>,
    pub fonts: Vec<String>,
    pub materials: Vec<Material>,
    pub root: Option<Pane>,
    pub groups: Vec<Group>,
}

/// A little-endian view of the file.
struct Reader<'a> {
    data: &'a [u8],
}

impl Reader<'_> {
    fn bytes(&self, at: usize, len: usize, what: &'static str) -> Result<&[u8]> {
        self.data
            .get(at..at.checked_add(len).ok_or(Error::OutOfBounds(what))?)
            .ok_or(Error::OutOfBounds(what))
    }
    fn u8(&self, at: usize) -> Result<u8> {
        Ok(self.bytes(at, 1, "field")?[0])
    }
    fn u16(&self, at: usize) -> Result<u16> {
        let b = self.bytes(at, 2, "field")?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }
    fn u32(&self, at: usize) -> Result<u32> {
        Ok(u32::from_le_bytes(
            self.bytes(at, 4, "field")?.try_into().unwrap(),
        ))
    }
    fn f32(&self, at: usize) -> Result<f32> {
        Ok(f32::from_bits(self.u32(at)?))
    }
    fn rgba(&self, at: usize) -> Result<[u8; 4]> {
        Ok(self.bytes(at, 4, "color")?.try_into().unwrap())
    }
    fn floats<const N: usize>(&self, at: usize) -> Result<[f32; N]> {
        let mut out = [0f32; N];
        for (i, v) in out.iter_mut().enumerate() {
            *v = self.f32(at + 4 * i)?;
        }
        Ok(out)
    }
    /// Reads a string that ends with a zero byte or at `max` bytes.
    fn string(&self, at: usize, max: usize) -> Result<String> {
        let rest = self.data.get(at..).ok_or(Error::OutOfBounds("name"))?;
        let rest = &rest[..rest.len().min(max)];
        let len = rest.iter().position(|&b| b == 0).unwrap_or(rest.len());
        Ok(String::from_utf8_lossy(&rest[..len]).into_owned())
    }
    /// Reads a list of names: a u32 count, then a u32 offset for each name
    /// from the start of the offsets.
    fn name_list(&self, section: usize) -> Result<Vec<String>> {
        let count = self.u16(section + 8)? as usize;
        let base = section + 12;
        (0..count)
            .map(|i| self.string(base + self.u32(base + 4 * i)? as usize, usize::MAX))
            .collect()
    }
}

/// Reads a layout.
pub fn parse(data: &[u8]) -> Result<Layout> {
    let r = Reader { data };
    if data.len() < 0x14 || &data[..4] != MAGIC {
        return Err(Error::BadMagic("FLYT header"));
    }
    let mut at = r.u16(6)? as usize;
    let count = r.u16(0x10)? as usize;

    let mut layout = Layout {
        name: String::new(),
        width: 0.0,
        height: 0.0,
        textures: Vec::new(),
        fonts: Vec::new(),
        materials: Vec::new(),
        root: None,
        groups: Vec::new(),
    };
    // The open panes and groups. The last one gets the next children.
    let mut panes: Vec<Pane> = Vec::new();
    let mut last_pane: Option<Pane> = None;
    let mut groups: Vec<Group> = Vec::new();
    let mut last_group: Option<Group> = None;

    for _ in 0..count {
        let magic: [u8; 4] = r.bytes(at, 4, "section")?.try_into().unwrap();
        let size = r.u32(at + 4)? as usize;
        if size < 8 {
            return Err(Error::OutOfBounds("section size"));
        }
        r.bytes(at, size, "section")?;
        match &magic {
            b"lyt1" => {
                layout.width = r.f32(at + 0x0c)?;
                layout.height = r.f32(at + 0x10)?;
                layout.name = r.string(at + 0x1c, size - 0x1c)?;
            }
            b"txl1" => layout.textures = r.name_list(at)?,
            b"fnl1" => layout.fonts = r.name_list(at)?,
            b"mat1" => layout.materials = materials(&r, at)?,
            b"pan1" | b"pic1" | b"txt1" | b"wnd1" | b"bnd1" | b"prt1" | b"ali1" => {
                if let Some(pane) = last_pane.take() {
                    attach_pane(&mut panes, &mut layout.root, pane)?;
                }
                last_pane = Some(pane(&r, at, &magic)?);
            }
            b"pas1" => panes.push(last_pane.take().ok_or(Error::BadTree("panes"))?),
            b"pae1" => {
                if let Some(pane) = last_pane.take() {
                    attach_pane(&mut panes, &mut layout.root, pane)?;
                }
                last_pane = Some(panes.pop().ok_or(Error::BadTree("panes"))?);
            }
            b"grp1" => {
                if let Some(group) = last_group.take() {
                    attach_group(&mut groups, &mut layout.groups, group);
                }
                last_group = Some(group(&r, at)?);
            }
            b"grs1" => groups.push(last_group.take().ok_or(Error::BadTree("groups"))?),
            b"gre1" => {
                if let Some(group) = last_group.take() {
                    attach_group(&mut groups, &mut layout.groups, group);
                }
                last_group = Some(groups.pop().ok_or(Error::BadTree("groups"))?);
            }
            _ => {}
        }
        at += size;
    }
    if let Some(pane) = last_pane {
        attach_pane(&mut panes, &mut layout.root, pane)?;
    }
    if let Some(group) = last_group {
        attach_group(&mut groups, &mut layout.groups, group);
    }
    if !panes.is_empty() {
        return Err(Error::BadTree("panes"));
    }
    if !groups.is_empty() {
        return Err(Error::BadTree("groups"));
    }
    Ok(layout)
}

/// Adds a finished pane to the open parent, or makes it the root.
fn attach_pane(open: &mut [Pane], root: &mut Option<Pane>, pane: Pane) -> Result<()> {
    match open.last_mut() {
        Some(parent) => parent.children.push(pane),
        None if root.is_none() => *root = Some(pane),
        None => return Err(Error::BadTree("panes")),
    }
    Ok(())
}

fn attach_group(open: &mut [Group], top: &mut Vec<Group>, group: Group) {
    match open.last_mut() {
        Some(parent) => parent.children.push(group),
        None => top.push(group),
    }
}

fn materials(r: &Reader<'_>, section: usize) -> Result<Vec<Material>> {
    let count = r.u16(section + 8)? as usize;
    (0..count)
        .map(|i| {
            let at = section + r.u32(section + 12 + 4 * i)? as usize;
            let flags = r.u32(at + MATERIAL_NAME_LEN)?;
            let maps = (flags & 3) as usize;
            let first = at + 0x2c;
            let textures = (0..maps)
                .map(|m| {
                    Ok(TexMap {
                        texture: r.u16(first + 4 * m)?,
                        wrap_s: r.u8(first + 4 * m + 2)?,
                        wrap_t: r.u8(first + 4 * m + 3)?,
                    })
                })
                .collect::<Result<_>>()?;
            Ok(Material {
                name: r.string(at, MATERIAL_NAME_LEN)?,
                flags,
                black: r.rgba(at + 0x24)?,
                white: r.rgba(at + 0x28)?,
                textures,
            })
        })
        .collect()
}

fn pane(r: &Reader<'_>, at: usize, magic: &[u8; 4]) -> Result<Pane> {
    let flags = r.u8(at + 8)?;
    let body = at + PANE_LEN;
    let kind = match magic {
        b"pic1" => {
            let mut vertex_colors = [[0u8; 4]; 4];
            for (i, c) in vertex_colors.iter_mut().enumerate() {
                *c = r.rgba(body + 4 * i)?;
            }
            let count = r.u8(body + 0x12)? as usize;
            let uvs = (0..count)
                .map(|i| {
                    let f: [f32; 8] = r.floats(body + 0x14 + 32 * i)?;
                    Ok([[f[0], f[1]], [f[2], f[3]], [f[4], f[5]], [f[6], f[7]]])
                })
                .collect::<Result<_>>()?;
            Kind::Picture {
                material: r.u16(body + 0x10)?,
                vertex_colors,
                uvs,
            }
        }
        b"txt1" => {
            let len = r.u16(body + 2)? as usize;
            let text_at = at + r.u32(body + 0x10)? as usize;
            let units: Vec<u16> = r
                .bytes(text_at, len, "text")?
                .as_chunks::<2>()
                .0
                .iter()
                .map(|c| u16::from_le_bytes(*c))
                .take_while(|&u| u != 0)
                .collect();
            Kind::Text {
                material: r.u16(body + 4)?,
                font: r.u16(body + 6)?,
                text: String::from_utf16_lossy(&units),
                font_size: r.floats(body + 0x1c)?,
                char_space: r.f32(body + 0x24)?,
                line_space: r.f32(body + 0x28)?,
                position: r.u8(body + 8)?,
                alignment: r.u8(body + 9)?,
                top_color: r.rgba(body + 0x14)?,
                bottom_color: r.rgba(body + 0x18)?,
            }
        }
        b"prt1" => {
            let properties = r.u32(body)? as usize;
            Kind::Part {
                magnify: r.floats(body + 4)?,
                layout: r.string(body + 12 + 40 * properties, usize::MAX)?,
            }
        }
        b"wnd1" => Kind::Window,
        b"bnd1" => Kind::Bounding,
        b"ali1" => Kind::Alignment,
        _ => Kind::Null,
    };
    Ok(Pane {
        name: r.string(at + 12, NAME_LEN)?,
        kind,
        visible: flags & 1 != 0,
        flags,
        origin: r.u8(at + 9)?,
        alpha: r.u8(at + 10)?,
        translate: r.floats(at + 0x2c)?,
        rotate: r.floats(at + 0x38)?,
        scale: r.floats(at + 0x44)?,
        size: r.floats(at + 0x4c)?,
        children: Vec::new(),
    })
}

fn group(r: &Reader<'_>, at: usize) -> Result<Group> {
    let count = r.u16(at + 8 + GROUP_NAME_LEN)? as usize;
    let first = at + 8 + GROUP_NAME_LEN + 2;
    Ok(Group {
        name: r.string(at + 8, GROUP_NAME_LEN)?,
        panes: (0..count)
            .map(|i| r.string(first + NAME_LEN * i, NAME_LEN))
            .collect::<Result<_>>()?,
        children: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn section(magic: &[u8; 4], body: &[u8]) -> Vec<u8> {
        let mut out = magic.to_vec();
        out.extend(((body.len() + 8) as u32).to_le_bytes());
        out.extend(body);
        out
    }

    fn name(text: &str, len: usize) -> Vec<u8> {
        let mut out = text.as_bytes().to_vec();
        out.resize(len, 0);
        out
    }

    fn floats(values: &[f32]) -> Vec<u8> {
        values.iter().flat_map(|v| v.to_le_bytes()).collect()
    }

    /// Builds the common block of a pane.
    fn pane_body(pane_name: &str, flags: u8, x: f32, w: f32) -> Vec<u8> {
        let mut out = vec![flags, 0, 255, 0];
        out.extend(name(pane_name, NAME_LEN));
        out.extend([0; 8]);
        out.extend(floats(&[x, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, w, 10.0]));
        out
    }

    fn name_list(names: &[&str]) -> Vec<u8> {
        let mut out = (names.len() as u32).to_le_bytes().to_vec();
        let mut strings = Vec::new();
        for n in names {
            out.extend(((4 * names.len() + strings.len()) as u32).to_le_bytes());
            strings.extend(n.as_bytes());
            strings.push(0);
        }
        out.extend(strings);
        out.resize(out.len().next_multiple_of(4), 0);
        out
    }

    fn layout() -> Vec<u8> {
        let mut sections = Vec::new();
        let mut lyt1 = vec![1, 0, 0, 0];
        lyt1.extend(floats(&[1920.0, 1080.0, 0.0, 0.0]));
        lyt1.extend(name("menu", 8));
        sections.push(section(b"lyt1", &lyt1));
        sections.push(section(b"txl1", &name_list(&["bg^q", "btn^q"])));
        sections.push(section(b"fnl1", &name_list(&["main.bffnt"])));

        let mut material = name("bg", MATERIAL_NAME_LEN);
        material.extend(0x15u32.to_le_bytes());
        material.extend([0, 2, 4, 8]);
        material.extend([0, 0, 0, 0]);
        material.extend([255; 4]);
        material.extend([1, 0, 4, 4]);
        let mut mat1 = 1u32.to_le_bytes().to_vec();
        mat1.extend(16u32.to_le_bytes());
        mat1.extend(material);
        sections.push(section(b"mat1", &mat1));

        sections.push(section(b"pan1", &pane_body("RootPane", 1, 0.0, 1920.0)));
        sections.push(section(b"pas1", &[]));
        let mut pic = pane_body("bg", 1, 5.0, 100.0);
        pic.extend([255; 16]);
        pic.extend(0u16.to_le_bytes());
        pic.extend([1, 0]);
        pic.extend(floats(&[0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 1.0, 1.0]));
        sections.push(section(b"pic1", &pic));

        let mut txt = pane_body("label", 0, -5.0, 50.0);
        let text_offset = (PANE_LEN + 0x40) as u32;
        txt.extend(6u16.to_le_bytes());
        txt.extend(6u16.to_le_bytes());
        txt.extend(0u16.to_le_bytes());
        txt.extend(0u16.to_le_bytes());
        txt.extend([4, 1, 0, 0]);
        txt.extend(0f32.to_le_bytes());
        txt.extend(text_offset.to_le_bytes());
        txt.extend([255; 8]);
        txt.extend(floats(&[32.0, 32.0, 1.0, 2.0]));
        txt.resize(text_offset as usize - 8, 0);
        txt.extend([b'O', 0, b'K', 0, 0, 0]);
        txt.resize(txt.len().next_multiple_of(4), 0);
        sections.push(section(b"txt1", &txt));

        let mut prt = pane_body("part", 1, 0.0, 30.0);
        prt.extend(0u32.to_le_bytes());
        prt.extend(floats(&[1.0, 1.0]));
        // A part name can be longer than a pane name.
        prt.extend(name("UI_Parts_MonologueSelect_A1", 28));
        sections.push(section(b"prt1", &prt));
        sections.push(section(b"pae1", &[]));

        let mut grp = name("RootGroup", GROUP_NAME_LEN);
        grp.extend(0u16.to_le_bytes());
        sections.push(section(b"grp1", &grp));
        sections.push(section(b"grs1", &[]));
        let mut all = name("all", GROUP_NAME_LEN);
        all.extend(2u16.to_le_bytes());
        all.extend(name("bg", NAME_LEN));
        all.extend(name("label", NAME_LEN));
        sections.push(section(b"grp1", &all));
        sections.push(section(b"gre1", &[]));
        sections.push(section(b"usd1", &[0; 8]));

        let body: Vec<u8> = sections.concat();
        let mut out = MAGIC.to_vec();
        out.extend([0xff, 0xfe]);
        out.extend(0x14u16.to_le_bytes());
        out.extend(0x0806_0000u32.to_le_bytes());
        out.extend(((0x14 + body.len()) as u32).to_le_bytes());
        out.extend((sections.len() as u16).to_le_bytes());
        out.extend([0, 0]);
        out.extend(body);
        out
    }

    #[test]
    fn reads_a_layout() {
        let l = parse(&layout()).unwrap();
        assert_eq!(l.name, "menu");
        assert_eq!((l.width, l.height), (1920.0, 1080.0));
        assert_eq!(l.textures, ["bg^q", "btn^q"]);
        assert_eq!(l.fonts, ["main.bffnt"]);
        assert_eq!(l.materials.len(), 1);
        let m = &l.materials[0];
        assert_eq!(m.name, "bg");
        assert_eq!(m.white, [255; 4]);
        assert_eq!(
            m.textures,
            [TexMap {
                texture: 1,
                wrap_s: 4,
                wrap_t: 4
            }]
        );

        let root = l.root.unwrap();
        assert_eq!(root.name, "RootPane");
        assert_eq!(root.kind, Kind::Null);
        assert_eq!(root.size, [1920.0, 10.0]);
        let names: Vec<&str> = root.children.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["bg", "label", "part"]);
        let bg = &root.children[0];
        assert_eq!(bg.translate, [5.0, 0.0, 0.0]);
        assert!(bg.visible);
        assert_eq!(
            bg.kind,
            Kind::Picture {
                material: 0,
                vertex_colors: [[255; 4]; 4],
                uvs: vec![[[0.0, 0.0], [1.0, 0.0], [0.0, 1.0], [1.0, 1.0]]],
            }
        );
        let label = &root.children[1];
        assert!(!label.visible);
        match &label.kind {
            Kind::Text {
                text,
                font_size,
                position,
                alignment,
                ..
            } => {
                assert_eq!(text, "OK");
                assert_eq!(*font_size, [32.0, 32.0]);
                assert_eq!((*position, *alignment), (4, 1));
            }
            other => panic!("not text: {other:?}"),
        }
        assert_eq!(
            root.children[2].kind,
            Kind::Part {
                layout: "UI_Parts_MonologueSelect_A1".to_string(),
                magnify: [1.0, 1.0]
            }
        );

        assert_eq!(l.groups.len(), 1);
        assert_eq!(l.groups[0].name, "RootGroup");
        assert_eq!(l.groups[0].children[0].name, "all");
        assert_eq!(l.groups[0].children[0].panes, ["bg", "label"]);
    }

    #[test]
    fn writes_json() {
        let json = serde_json::to_value(parse(&layout()).unwrap()).unwrap();
        assert_eq!(json["root"]["type"], "null");
        assert_eq!(json["root"]["children"][1]["type"], "text");
        assert_eq!(json["root"]["children"][1]["text"], "OK");
    }

    #[test]
    fn rejects_bad_layouts() {
        assert!(matches!(parse(b"FLAN"), Err(Error::BadMagic(_))));
        let bytes = layout();
        assert!(parse(&bytes[..bytes.len() - 20]).is_err());
        // A pae1 with no pas1 does not close.
        let mut open = bytes.clone();
        let at = open.windows(4).position(|w| w == b"pas1").unwrap();
        open[at..at + 4].copy_from_slice(b"usd1");
        assert_eq!(parse(&open), Err(Error::BadTree("panes")));
    }
}
