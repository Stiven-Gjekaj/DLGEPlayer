//! Reads and writes CRI `@UTF` tables.
//!
//! A `@UTF` table holds the CPK header and the file lists.
//! All numbers in the table are big-endian.
//! The offsets in the table header start 8 bytes after the magic number.

use crate::error::{Error, Result};

const MAGIC: &[u8; 4] = b"@UTF";

const STORAGE_MASK: u8 = 0xf0;
const STORAGE_ZERO: u8 = 0x10;
const STORAGE_CONSTANT: u8 = 0x30;
const STORAGE_ROW: u8 = 0x50;
const TYPE_MASK: u8 = 0x0f;

/// One cell of a table.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    U8(u8),
    I8(i8),
    U16(u16),
    I16(i16),
    U32(u32),
    I32(i32),
    U64(u64),
    I64(i64),
    F32(f32),
    F64(f64),
    String(String),
    Data(Vec<u8>),
    /// The column has zero storage. The cell has no value.
    None,
}

impl Value {
    fn type_code(&self) -> Option<u8> {
        Some(match self {
            Value::U8(_) => 0x0,
            Value::I8(_) => 0x1,
            Value::U16(_) => 0x2,
            Value::I16(_) => 0x3,
            Value::U32(_) => 0x4,
            Value::I32(_) => 0x5,
            Value::U64(_) => 0x6,
            Value::I64(_) => 0x7,
            Value::F32(_) => 0x8,
            Value::F64(_) => 0x9,
            Value::String(_) => 0xa,
            Value::Data(_) => 0xb,
            Value::None => return None,
        })
    }

    /// Gives the value as an unsigned integer, if the value is an integer.
    pub fn as_u64(&self) -> Option<u64> {
        Some(match *self {
            Value::U8(v) => v.into(),
            Value::I8(v) => v as u64,
            Value::U16(v) => v.into(),
            Value::I16(v) => v as u64,
            Value::U32(v) => v.into(),
            Value::I32(v) => v as u64,
            Value::U64(v) => v,
            Value::I64(v) => v as u64,
            _ => return None,
        })
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_data(&self) -> Option<&[u8]> {
        match self {
            Value::Data(d) => Some(d),
            _ => None,
        }
    }
}

/// A `@UTF` table.
#[derive(Debug, Clone, PartialEq)]
pub struct Table {
    pub name: String,
    pub columns: Vec<String>,
    pub rows: Vec<Vec<Value>>,
}

impl Table {
    /// Gives the cell in `row` for the column with the name `column`.
    pub fn get(&self, row: usize, column: &str) -> Option<&Value> {
        let index = self.columns.iter().position(|c| c == column)?;
        self.rows
            .get(row)?
            .get(index)
            .filter(|v| **v != Value::None)
    }

    pub fn get_u64(&self, row: usize, column: &str) -> Option<u64> {
        self.get(row, column)?.as_u64()
    }

    pub fn get_str(&self, row: usize, column: &str) -> Option<&str> {
        self.get(row, column)?.as_str()
    }

    /// Reads a table. The data can start with the XOR mask that some
    /// CPK files use.
    pub fn parse(data: &[u8]) -> Result<Table> {
        if data.len() >= 4 && &data[..4] != MAGIC {
            let plain = unmask(data);
            if &plain[..4] == MAGIC {
                return parse_plain(&plain);
            }
        }
        parse_plain(data)
    }

    /// Writes the table. Each column has row storage, or zero storage
    /// if all of its cells are `Value::None`.
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        write(self)
    }
}

/// Removes or adds the XOR mask. The mask is its own inverse.
pub fn unmask(data: &[u8]) -> Vec<u8> {
    let mut m: u32 = 0x0000_655f;
    let t: u32 = 0x0000_4115;
    data.iter()
        .map(|b| {
            let out = b ^ (m as u8);
            m = m.wrapping_mul(t);
            out
        })
        .collect()
}

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
        Ok(self.bytes(at, 1, "utf cell")?[0])
    }

    fn u16(&self, at: usize) -> Result<u16> {
        Ok(u16::from_be_bytes(
            self.bytes(at, 2, "utf cell")?.try_into().unwrap(),
        ))
    }

    fn u32(&self, at: usize) -> Result<u32> {
        Ok(u32::from_be_bytes(
            self.bytes(at, 4, "utf cell")?.try_into().unwrap(),
        ))
    }

    fn u64(&self, at: usize) -> Result<u64> {
        Ok(u64::from_be_bytes(
            self.bytes(at, 8, "utf cell")?.try_into().unwrap(),
        ))
    }

    fn cstr(&self, at: usize) -> Result<String> {
        let rest = self
            .data
            .get(at..)
            .ok_or(Error::OutOfBounds("utf string"))?;
        let end = rest
            .iter()
            .position(|&b| b == 0)
            .ok_or(Error::OutOfBounds("utf string"))?;
        Ok(String::from_utf8_lossy(&rest[..end]).into_owned())
    }
}

fn parse_plain(data: &[u8]) -> Result<Table> {
    if data.len() < 0x20 || &data[..4] != MAGIC {
        return Err(Error::BadMagic("@UTF table"));
    }
    let r = Reader { data };
    let size = r.u32(4)? as usize;
    let body = Reader {
        data: r.bytes(8, size, "utf table size")?,
    };
    let rows_offset = body.u32(0)? as usize;
    let strings_offset = body.u32(4)? as usize;
    let data_offset = body.u32(8)? as usize;
    let name_offset = body.u32(12)? as usize;
    let column_count = body.u16(16)? as usize;
    let row_length = body.u16(18)? as usize;
    let row_count = body.u32(20)? as usize;

    let string = |offset: u32| body.cstr(strings_offset + offset as usize);

    struct Column {
        flags: u8,
        constant: Value,
    }

    let mut columns = Vec::with_capacity(column_count);
    let mut names = Vec::with_capacity(column_count);
    let mut at = 24;
    for _ in 0..column_count {
        let flags = body.u8(at)?;
        names.push(string(body.u32(at + 1)?)?);
        at += 5;
        let constant = match flags & STORAGE_MASK {
            STORAGE_CONSTANT => {
                let (value, len) = read_value(&body, at, flags & TYPE_MASK, &string, data_offset)?;
                at += len;
                value
            }
            STORAGE_ZERO | STORAGE_ROW => Value::None,
            other => return Err(Error::Unsupported(format!("utf storage 0x{other:02x}"))),
        };
        columns.push(Column { flags, constant });
    }

    let mut rows = Vec::with_capacity(row_count);
    for row in 0..row_count {
        let mut at = rows_offset + row * row_length;
        let mut cells = Vec::with_capacity(column_count);
        for column in &columns {
            let value = match column.flags & STORAGE_MASK {
                STORAGE_ROW => {
                    let (value, len) =
                        read_value(&body, at, column.flags & TYPE_MASK, &string, data_offset)?;
                    at += len;
                    value
                }
                STORAGE_CONSTANT => column.constant.clone(),
                _ => Value::None,
            };
            cells.push(value);
        }
        rows.push(cells);
    }

    Ok(Table {
        name: string(name_offset as u32)?,
        columns: names,
        rows,
    })
}

fn read_value(
    r: &Reader<'_>,
    at: usize,
    type_code: u8,
    string: &dyn Fn(u32) -> Result<String>,
    data_offset: usize,
) -> Result<(Value, usize)> {
    Ok(match type_code {
        0x0 => (Value::U8(r.u8(at)?), 1),
        0x1 => (Value::I8(r.u8(at)? as i8), 1),
        0x2 => (Value::U16(r.u16(at)?), 2),
        0x3 => (Value::I16(r.u16(at)? as i16), 2),
        0x4 => (Value::U32(r.u32(at)?), 4),
        0x5 => (Value::I32(r.u32(at)? as i32), 4),
        0x6 => (Value::U64(r.u64(at)?), 8),
        0x7 => (Value::I64(r.u64(at)? as i64), 8),
        0x8 => (Value::F32(f32::from_bits(r.u32(at)?)), 4),
        0x9 => (Value::F64(f64::from_bits(r.u64(at)?)), 8),
        0xa => (Value::String(string(r.u32(at)?)?), 4),
        0xb => {
            let offset = r.u32(at)? as usize;
            let size = r.u32(at + 4)? as usize;
            let bytes = r.bytes(data_offset + offset, size, "utf data cell")?;
            (Value::Data(bytes.to_vec()), 8)
        }
        other => return Err(Error::Unsupported(format!("utf type 0x{other:x}"))),
    })
}

fn write(table: &Table) -> Result<Vec<u8>> {
    let column_count = table.columns.len();
    for row in &table.rows {
        if row.len() != column_count {
            return Err(Error::Unsupported(
                "row length differs from column count".into(),
            ));
        }
    }

    // Find the type of each column. All cells of a column must agree.
    let mut types: Vec<Option<u8>> = vec![None; column_count];
    for row in &table.rows {
        for (i, cell) in row.iter().enumerate() {
            match (types[i], cell.type_code()) {
                (_, None) => {}
                (None, code) => types[i] = code,
                (Some(a), Some(b)) if a != b => {
                    return Err(Error::Unsupported(format!(
                        "column {} has mixed types",
                        table.columns[i]
                    )));
                }
                _ => {}
            }
        }
    }

    let mut strings = Vec::new();
    let mut add_string = |s: &str| -> u32 {
        let offset = strings.len() as u32;
        strings.extend_from_slice(s.as_bytes());
        strings.push(0);
        offset
    };
    add_string("<NULL>");
    let name_offset = add_string(&table.name);

    let mut schema = Vec::new();
    let mut row_length = 0usize;
    for (i, name) in table.columns.iter().enumerate() {
        let name_at = add_string(name);
        match types[i] {
            Some(code) => {
                schema.push(STORAGE_ROW | code);
                row_length += cell_size(code);
            }
            None => schema.push(STORAGE_ZERO),
        }
        schema.extend_from_slice(&name_at.to_be_bytes());
    }

    let mut data = Vec::new();
    let mut rows = Vec::new();
    for row in &table.rows {
        for (i, cell) in row.iter().enumerate() {
            if types[i].is_none() {
                continue;
            }
            match cell {
                Value::U8(v) => rows.push(*v),
                Value::I8(v) => rows.push(*v as u8),
                Value::U16(v) => rows.extend_from_slice(&v.to_be_bytes()),
                Value::I16(v) => rows.extend_from_slice(&v.to_be_bytes()),
                Value::U32(v) => rows.extend_from_slice(&v.to_be_bytes()),
                Value::I32(v) => rows.extend_from_slice(&v.to_be_bytes()),
                Value::U64(v) => rows.extend_from_slice(&v.to_be_bytes()),
                Value::I64(v) => rows.extend_from_slice(&v.to_be_bytes()),
                Value::F32(v) => rows.extend_from_slice(&v.to_bits().to_be_bytes()),
                Value::F64(v) => rows.extend_from_slice(&v.to_bits().to_be_bytes()),
                Value::String(s) => rows.extend_from_slice(&add_string(s).to_be_bytes()),
                Value::Data(d) => {
                    rows.extend_from_slice(&(data.len() as u32).to_be_bytes());
                    rows.extend_from_slice(&(d.len() as u32).to_be_bytes());
                    data.extend_from_slice(d);
                }
                Value::None => {
                    return Err(Error::Unsupported(format!(
                        "column {} has a missing cell",
                        table.columns[i]
                    )));
                }
            }
        }
    }

    let rows_offset = 24 + schema.len();
    let strings_offset = rows_offset + rows.len();
    let data_offset = strings_offset + strings.len();

    let mut body = Vec::new();
    body.extend_from_slice(&(rows_offset as u32).to_be_bytes());
    body.extend_from_slice(&(strings_offset as u32).to_be_bytes());
    body.extend_from_slice(&(data_offset as u32).to_be_bytes());
    body.extend_from_slice(&name_offset.to_be_bytes());
    body.extend_from_slice(&(column_count as u16).to_be_bytes());
    body.extend_from_slice(&(row_length as u16).to_be_bytes());
    body.extend_from_slice(&(table.rows.len() as u32).to_be_bytes());
    body.extend_from_slice(&schema);
    body.extend_from_slice(&rows);
    body.extend_from_slice(&strings);
    body.extend_from_slice(&data);

    let mut out = Vec::with_capacity(body.len() + 8);
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&(body.len() as u32).to_be_bytes());
    out.extend_from_slice(&body);
    Ok(out)
}

fn cell_size(type_code: u8) -> usize {
    match type_code {
        0x0 | 0x1 => 1,
        0x2 | 0x3 => 2,
        0x4 | 0x5 | 0x8 | 0xa => 4,
        _ => 8,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Table {
        Table {
            name: "CpkTocInfo".into(),
            columns: vec![
                "DirName".into(),
                "FileName".into(),
                "FileSize".into(),
                "FileOffset".into(),
                "Flag".into(),
                "Blob".into(),
                "Unused".into(),
            ],
            rows: vec![
                vec![
                    Value::String("dir".into()),
                    Value::String("a.dat".into()),
                    Value::U32(10),
                    Value::U64(0x800),
                    Value::I8(-1),
                    Value::Data(vec![1, 2, 3]),
                    Value::None,
                ],
                vec![
                    Value::String("".into()),
                    Value::String("b.dat".into()),
                    Value::U32(20),
                    Value::U64(0x1000),
                    Value::I8(5),
                    Value::Data(vec![]),
                    Value::None,
                ],
            ],
        }
    }

    #[test]
    fn round_trip() {
        let table = sample();
        let bytes = table.to_bytes().unwrap();
        assert_eq!(&bytes[..4], b"@UTF");
        assert_eq!(Table::parse(&bytes).unwrap(), table);
    }

    #[test]
    fn reads_masked_table() {
        let table = sample();
        let masked = unmask(&table.to_bytes().unwrap());
        assert_ne!(&masked[..4], b"@UTF");
        assert_eq!(Table::parse(&masked).unwrap(), table);
    }

    #[test]
    fn get_finds_cells_by_column_name() {
        let table = sample();
        assert_eq!(table.get_str(1, "FileName"), Some("b.dat"));
        assert_eq!(table.get_u64(0, "FileOffset"), Some(0x800));
        assert_eq!(table.get(0, "Unused"), None);
        assert_eq!(table.get(0, "Missing"), None);
    }

    #[test]
    fn reads_constant_storage() {
        // Build a one-column table by hand. The column has constant
        // storage, so each row gives the same value and the rows are empty.
        let strings = b"<NULL>\0T\0C\0";
        let mut body = Vec::new();
        let schema = [STORAGE_CONSTANT | 0x4, 0, 0, 0, 9, 0, 0, 0, 42];
        let rows_offset = 24 + schema.len() as u32;
        body.extend_from_slice(&rows_offset.to_be_bytes());
        body.extend_from_slice(&rows_offset.to_be_bytes());
        body.extend_from_slice(&(rows_offset + strings.len() as u32).to_be_bytes());
        body.extend_from_slice(&7u32.to_be_bytes());
        body.extend_from_slice(&1u16.to_be_bytes());
        body.extend_from_slice(&0u16.to_be_bytes());
        body.extend_from_slice(&3u32.to_be_bytes());
        body.extend_from_slice(&schema);
        body.extend_from_slice(strings);
        let mut bytes = b"@UTF".to_vec();
        bytes.extend_from_slice(&(body.len() as u32).to_be_bytes());
        bytes.extend_from_slice(&body);

        let table = Table::parse(&bytes).unwrap();
        assert_eq!(table.name, "T");
        assert_eq!(table.columns, vec!["C".to_string()]);
        assert_eq!(table.rows, vec![vec![Value::U32(42)]; 3]);
    }

    #[test]
    fn rejects_bad_magic() {
        assert!(matches!(Table::parse(&[0u8; 64]), Err(Error::BadMagic(_))));
    }

    #[test]
    fn rejects_truncated_table() {
        let bytes = sample().to_bytes().unwrap();
        assert!(Table::parse(&bytes[..bytes.len() - 10]).is_err());
    }
}
