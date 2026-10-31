//! Reads `BIN_` tables, for example `static/chara.bin`.
//!
//! All numbers are little-endian. The header:
//!
//! - 0x00: the magic `BIN_`.
//! - 0x06: the number of rows, as a u16.
//! - 0x08: the number of integer slots, as a u32. It is a multiple of 2.
//! - 0x0c: the number of strings, as a u32.
//! - 0x10: the offset of the integers. Each integer is an i16.
//! - 0x14: the offset of the string offsets. Each is a u32 from 0x18.
//! - 0x18: the offset of the strings. Each string ends with a zero byte.
//!
//! The first string gives the type of each column: `s` for a string and
//! `i` for an integer. The table keeps the values column by column: all
//! rows of the first string column, then all rows of the next one. The
//! integers are kept in the same way. The strings are Shift-JIS.

use crate::{Error, Result};

pub const MAGIC: &[u8; 4] = b"BIN_";

/// One cell of a table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Cell {
    Int(i16),
    Text(String),
}

impl Cell {
    pub fn as_int(&self) -> Option<i16> {
        match self {
            Cell::Int(v) => Some(*v),
            Cell::Text(_) => None,
        }
    }

    pub fn as_text(&self) -> Option<&str> {
        match self {
            Cell::Text(s) => Some(s),
            Cell::Int(_) => None,
        }
    }
}

/// A table with its column types and its rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Table {
    /// The type of each column: `s` or `i`.
    pub columns: String,
    pub rows: Vec<Vec<Cell>>,
}

impl Table {
    pub fn parse(data: &[u8]) -> Result<Table> {
        if data.len() < 0x1c || &data[..4] != MAGIC {
            return Err(Error::BadMagic("BIN_ table"));
        }
        let u32_at = |at: usize| -> Result<u32> {
            Ok(u32::from_le_bytes(
                data.get(at..at + 4)
                    .ok_or(Error::OutOfBounds("table field"))?
                    .try_into()
                    .unwrap(),
            ))
        };
        let row_count = u16::from_le_bytes([data[6], data[7]]) as usize;
        let int_slots = u32_at(0x08)? as usize;
        let string_count = u32_at(0x0c)? as usize;
        let ints_at = u32_at(0x10)? as usize;
        let offsets_at = u32_at(0x14)? as usize;
        let strings_at = u32_at(0x18)? as usize;
        if string_count > data.len() / 4 {
            return Err(Error::OutOfBounds("string count"));
        }

        let string = |index: usize| -> Result<String> {
            let offset = u32_at(offsets_at + 4 * index)? as usize;
            let start = strings_at
                .checked_add(offset)
                .ok_or(Error::OutOfBounds("string"))?;
            let rest = data.get(start..).ok_or(Error::OutOfBounds("string"))?;
            let end = rest
                .iter()
                .position(|&b| b == 0)
                .ok_or(Error::OutOfBounds("string"))?;
            let (text, _, _) = encoding_rs::SHIFT_JIS.decode(&rest[..end]);
            Ok(text.into_owned())
        };
        let int = |index: usize| -> Result<i16> {
            if index >= int_slots {
                return Err(Error::OutOfBounds("integer"));
            }
            let at = ints_at + 2 * index;
            let b = data.get(at..at + 2).ok_or(Error::OutOfBounds("integer"))?;
            Ok(i16::from_le_bytes([b[0], b[1]]))
        };

        if string_count == 0 {
            return Err(Error::OutOfBounds("column types"));
        }
        let columns = string(0)?;
        if let Some(bad) = columns.chars().find(|c| !matches!(c, 's' | 'i')) {
            return Err(Error::Unsupported(format!("column type {bad:?}")));
        }
        let text_columns = columns.chars().filter(|&c| c == 's').count();
        if 1 + text_columns * row_count != string_count {
            return Err(Error::Unsupported(format!(
                "{string_count} strings for {row_count} rows of {columns:?}"
            )));
        }

        let mut rows = vec![Vec::with_capacity(columns.len()); row_count];
        let (mut text_column, mut int_column) = (0, 0);
        for kind in columns.chars() {
            for (r, row) in rows.iter_mut().enumerate() {
                row.push(if kind == 's' {
                    Cell::Text(string(1 + text_column * row_count + r)?)
                } else {
                    Cell::Int(int(int_column * row_count + r)?)
                });
            }
            if kind == 's' {
                text_column += 1;
            } else {
                int_column += 1;
            }
        }
        Ok(Table { columns, rows })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds a table from rows. Each row has the types of `columns`.
    fn build(columns: &str, rows: &[Vec<Cell>]) -> Vec<u8> {
        let mut strings = vec![columns.as_bytes().to_vec()];
        let mut ints = Vec::new();
        for (c, kind) in columns.chars().enumerate() {
            for row in rows {
                match (kind, &row[c]) {
                    ('s', Cell::Text(s)) => {
                        let (bytes, _, _) = encoding_rs::SHIFT_JIS.encode(s);
                        strings.push(bytes.into_owned());
                    }
                    ('i', Cell::Int(v)) => ints.push(*v),
                    _ => panic!("the row does not agree with the column types"),
                }
            }
        }
        let slots = ints.len().next_multiple_of(2);
        ints.resize(slots, 0);

        let ints_at = 0x1cu32;
        let offsets_at = ints_at + 2 * slots as u32;
        let strings_at = offsets_at + 4 * strings.len() as u32;
        let mut out = MAGIC.to_vec();
        out.extend(0u16.to_le_bytes());
        out.extend((rows.len() as u16).to_le_bytes());
        for v in [
            slots as u32,
            strings.len() as u32,
            ints_at,
            offsets_at,
            strings_at,
        ] {
            out.extend(v.to_le_bytes());
        }
        for v in &ints {
            out.extend(v.to_le_bytes());
        }
        let mut offset = 0u32;
        for s in &strings {
            out.extend(offset.to_le_bytes());
            offset += s.len() as u32 + 1;
        }
        for s in &strings {
            out.extend(s);
            out.push(0);
        }
        out
    }

    fn text(s: &str) -> Cell {
        Cell::Text(s.into())
    }

    #[test]
    fn reads_columns_in_column_order() {
        let rows = vec![
            vec![Cell::Int(0), text("#Name[2]"), text("her")],
            vec![Cell::Int(1), text("アヤト"), text("aya")],
            vec![Cell::Int(-1), text(""), text("kan")],
        ];
        let bytes = build("iss", &rows);
        let table = Table::parse(&bytes).unwrap();
        assert_eq!(table.columns, "iss");
        assert_eq!(table.rows, rows);
        assert_eq!(table.rows[1][1].as_text(), Some("アヤト"));
        assert_eq!(table.rows[2][0].as_int(), Some(-1));
    }

    #[test]
    fn keeps_the_values_column_by_column() {
        // Two rows of "ii". The values are kept as 1, 3 and then 2, 4.
        let rows = vec![
            vec![Cell::Int(1), Cell::Int(2)],
            vec![Cell::Int(3), Cell::Int(4)],
        ];
        let bytes = build("ii", &rows);
        let values: Vec<i16> = bytes[0x1c..0x24]
            .chunks(2)
            .map(|b| i16::from_le_bytes([b[0], b[1]]))
            .collect();
        assert_eq!(values, [1, 3, 2, 4]);
        assert_eq!(Table::parse(&bytes).unwrap().rows, rows);
    }

    #[test]
    fn rejects_bad_tables() {
        assert!(matches!(
            Table::parse(&[0u8; 0x20]),
            Err(Error::BadMagic(_))
        ));
        let bytes = build("si", &[vec![text("a"), Cell::Int(1)]]);
        for len in [0x10, 0x20, bytes.len() - 1] {
            assert!(Table::parse(&bytes[..len]).is_err(), "length {len}");
        }
        let mut wrong = bytes.clone();
        // Say that the table has 2 rows. The strings do not agree.
        wrong[6] = 2;
        assert!(matches!(Table::parse(&wrong), Err(Error::Unsupported(_))));
        let mut wrong = build("sx", &[]);
        wrong[6] = 0;
        assert!(matches!(Table::parse(&wrong), Err(Error::Unsupported(_))));
    }
}
