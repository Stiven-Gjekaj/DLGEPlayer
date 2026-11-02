//! The data that stays between chapters: the system flags, the finished
//! chapters, the seen event images, the read messages, and the name of the
//! heroine.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

/// The data of one player for both games.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Profile {
    /// The system flags that changed from their first values.
    pub system: BTreeMap<String, i32>,
    /// The chapters that reached their end.
    pub finished: BTreeSet<String>,
    /// The gallery numbers of the event images that the player saw.
    pub seen: BTreeSet<i32>,
    /// A hash of each message that the player saw.
    pub read: BTreeSet<u64>,
    /// The family name of the heroine. The scripts write `#Name[1]`.
    pub family_name: String,
    /// The given name of the heroine. The scripts write `#Name[2]`.
    pub given_name: String,
    /// The last pick of the character select, from 1 to 6.
    pub character: i32,
}

impl Default for Profile {
    fn default() -> Profile {
        Profile {
            system: BTreeMap::new(),
            finished: BTreeSet::new(),
            seen: BTreeSet::new(),
            read: BTreeSet::new(),
            family_name: "Komori".into(),
            given_name: "Yui".into(),
            character: 1,
        }
    }
}

impl Profile {
    /// Reads a profile from JSON. A missing field has its default value.
    pub fn parse(text: &str) -> Result<Profile, serde_json::Error> {
        serde_json::from_str(text)
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }
}
