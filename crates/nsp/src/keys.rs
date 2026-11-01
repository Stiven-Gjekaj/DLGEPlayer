//! Reads a `prod.keys` file. Each line is `name = hex`.

use std::collections::HashMap;

use crate::{Error, Result};

/// The keys of the console of the user.
#[derive(Clone, Default)]
pub struct Keys {
    keys: HashMap<String, Vec<u8>>,
}

impl std::fmt::Debug for Keys {
    /// Prints the names only. The values are secret.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut names: Vec<&String> = self.keys.keys().collect();
        names.sort();
        f.debug_struct("Keys").field("names", &names).finish()
    }
}

fn hex(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) {
        return None;
    }
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(text.get(i..i + 2)?, 16).ok())
        .collect()
}

impl Keys {
    /// Reads the text of a keys file. A line that is not `name = hex` with
    /// an even number of digits is ignored.
    pub fn parse(text: &str) -> Keys {
        let keys = text
            .lines()
            .filter_map(|line| {
                let (name, value) = line.split_once('=')?;
                Some((name.trim().to_ascii_lowercase(), hex(value.trim())?))
            })
            .collect();
        Keys { keys }
    }

    fn get<const N: usize>(&self, name: &str) -> Result<[u8; N]> {
        self.keys
            .get(name)
            .and_then(|v| v.as_slice().try_into().ok())
            .ok_or_else(|| Error::MissingKey(name.to_string()))
    }

    pub fn header_key(&self) -> Result<[u8; 32]> {
        self.get("header_key")
    }

    /// Gives the key area key of an index (0 application, 1 ocean,
    /// 2 system) and a key revision.
    pub fn key_area_key(&self, index: u8, revision: u8) -> Result<[u8; 16]> {
        let kind = match index {
            0 => "application",
            1 => "ocean",
            2 => "system",
            _ => return Err(Error::Unsupported(format!("key area index {index}"))),
        };
        self.get(&format!("key_area_key_{kind}_{revision:02x}"))
    }

    pub fn titlekek(&self, revision: u8) -> Result<[u8; 16]> {
        self.get(&format!("titlekek_{revision:02x}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_keys() {
        let keys = Keys::parse(
            "header_key = 000102030405060708090a0b0c0d0e0f000102030405060708090a0b0c0d0e0f\n\
             key_area_key_application_0a = FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF\n\
             master_kek_source_12 = 0123456789abcdef0123456789abcdef0\n\
             not a key line\n",
        );
        assert_eq!(keys.header_key().unwrap()[1], 1);
        assert_eq!(keys.key_area_key(0, 0x0a).unwrap(), [0xff; 16]);
        assert_eq!(
            keys.key_area_key(1, 0x0a),
            Err(Error::MissingKey("key_area_key_ocean_0a".into()))
        );
        assert!(matches!(keys.titlekek(0), Err(Error::MissingKey(_))));
        // The odd line with 33 digits is ignored.
        assert!(!format!("{keys:?}").contains("master_kek_source_12"));
    }

    #[test]
    fn does_not_print_values() {
        let keys = Keys::parse("titlekek_00 = 11223344556677881122334455667788");
        let text = format!("{keys:?}");
        assert!(text.contains("titlekek_00"));
        assert!(!text.contains("1122"));
    }
}
