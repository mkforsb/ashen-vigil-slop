//! Persistent progress (serialised to local storage by the web shell).

use crate::props::Key;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const SAVE_VERSION: u32 = 1;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Progress {
    pub version: u32,
    /// World-state flags: opened doors, taken items, lit pyres, defeated bosses...
    pub flags: BTreeSet<String>,
    pub keys: BTreeSet<Key>,
    pub cinders: u32,
    pub flasks: u8,
    pub flasks_max: u8,
    pub last_pyre: Option<String>,
    /// Dropped cinders: position and amount.
    pub bloodstain: Option<([f32; 3], u32)>,
    pub deaths: u32,
    pub kills: u32,
    pub playtime: f32,
}

impl Default for Progress {
    fn default() -> Self {
        Progress {
            version: SAVE_VERSION,
            flags: BTreeSet::new(),
            keys: BTreeSet::new(),
            cinders: 0,
            flasks: 0,
            flasks_max: 0,
            last_pyre: None,
            bloodstain: None,
            deaths: 0,
            kills: 0,
            playtime: 0.0,
        }
    }
}

impl Progress {
    pub fn has(&self, flag: &str) -> bool {
        self.flags.contains(flag)
    }

    pub fn set(&mut self, flag: &str) {
        self.flags.insert(flag.to_string());
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    pub fn from_json(s: &str) -> Option<Progress> {
        let p: Progress = serde_json::from_str(s).ok()?;
        (p.version == SAVE_VERSION).then_some(p)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let mut p = Progress::default();
        p.set("door_cell");
        p.keys.insert(Key::Cell);
        p.bloodstain = Some(([1.0, 2.0, 3.0], 120));
        let s = p.to_json();
        assert_eq!(Progress::from_json(&s), Some(p));
        assert_eq!(Progress::from_json("garbage"), None);
    }
}
