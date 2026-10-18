//! Isolated T13 experiment. This module is not approved for production adoption.
use std::collections::{BTreeMap, BTreeSet};
#[derive(Clone, Debug, PartialEq)]
pub struct Snapshot {
    pub hid: u16,
    pub flags: u8,
    pub logical: String,
    pub text: String,
}
#[derive(Clone, Debug, PartialEq)]
pub enum Output {
    Modifiers(u8),
    Key {
        snapshot: Snapshot,
        pressed: bool,
        repeat: bool,
    },
}
#[derive(Default)]
pub struct Keys {
    flags: u8,
    known_modifiers: u8,
    held: BTreeMap<u16, Snapshot>,
    // Primitive IDs remain until physical release. u16 bounds the set to 65536 IDs.
    suppressed: BTreeSet<u16>,
}
impl Keys {
    pub fn update(&mut self, mut snapshot: Snapshot, pressed: bool) -> Vec<Output> {
        if !self.suppressed.is_empty() {
            if pressed {
                self.suppressed.insert(snapshot.hid);
            } else {
                self.suppressed.remove(&snapshot.hid);
            }
            return Vec::new();
        }
        let bit = modifier(snapshot.hid);
        if !pressed && bit == 0 && !self.held.contains_key(&snapshot.hid) {
            return Vec::new();
        }
        if pressed
            && ((!self.held.contains_key(&snapshot.hid) && self.held.len() >= 64)
                || snapshot.logical.len() > 128
                || snapshot.text.len() > 128)
        {
            self.suppressed.insert(snapshot.hid);
            return self.cancel();
        }
        self.known_modifiers |= bit;
        let repeat = pressed && self.held.contains_key(&snapshot.hid);
        let mut previous = if pressed {
            self.held.insert(snapshot.hid, snapshot.clone())
        } else {
            self.held.remove(&snapshot.hid)
        };
        let held_modifiers = self
            .held
            .keys()
            .fold(0, |flags, hid| flags | modifier(*hid));
        snapshot.flags = (snapshot.flags & !self.known_modifiers) | held_modifiers;
        let mut out = Vec::new();
        if self.flags != snapshot.flags {
            self.flags = snapshot.flags;
            out.push(Output::Modifiers(self.flags));
        }
        if pressed {
            if let Some(held) = self.held.get_mut(&snapshot.hid) {
                held.flags = snapshot.flags;
            }
            out.push(Output::Key {
                snapshot,
                pressed: true,
                repeat,
            });
        } else if let Some(mut down) = previous.take() {
            down.flags = snapshot.flags;
            down.text.clear();
            out.push(Output::Key {
                snapshot: down,
                pressed: false,
                repeat: false,
            });
        }
        out
    }
    pub fn cancel(&mut self) -> Vec<Output> {
        let mut out = Vec::new();
        self.suppressed.extend(self.held.keys().copied());
        for (_, mut snapshot) in std::mem::take(&mut self.held) {
            snapshot.text.clear();
            out.push(Output::Key {
                snapshot,
                pressed: false,
                repeat: false,
            });
        }
        self.flags = 0;
        self.known_modifiers = 0;
        out.push(Output::Modifiers(0));
        out
    }
}
fn modifier(hid: u16) -> u8 {
    match hid {
        224 | 228 => 2,
        225 | 229 => 1,
        226 | 230 => 4,
        227 | 231 => 8,
        _ => 0,
    }
}
pub fn text_for(snapshot: &Snapshot, pressed: bool) -> Option<String> {
    (pressed
        && snapshot.flags & (2 | 8) == 0
        && !snapshot.text.is_empty()
        && !snapshot.text.starts_with("UIKeyInput")
        && snapshot.text.chars().all(|c| !c.is_control()))
    .then(|| snapshot.text.clone())
}
#[cfg(test)]
mod tests {
    use super::*;
    fn key(hid: u16, flags: u8) -> Snapshot {
        Snapshot {
            hid,
            flags,
            logical: "a".into(),
            text: "a".into(),
        }
    }
    #[test]
    fn ipad_command_control_distinct() {
        let mut keys = Keys::default();
        assert_eq!(keys.update(key(4, 8), true)[0], Output::Modifiers(8));
        assert_eq!(keys.update(key(4, 2), false)[0], Output::Modifiers(2));
    }
    #[test]
    fn ipad_repeat_release_once() {
        let mut keys = Keys::default();
        keys.update(key(4, 0), true);
        assert!(matches!(
            keys.update(key(4, 0), true)[0],
            Output::Key { repeat: true, .. }
        ));
        assert_eq!(keys.update(key(4, 0), false).len(), 1);
        assert!(keys.update(key(4, 0), false).is_empty());
    }
    #[test]
    fn ipad_cancel_and_late_up() {
        let mut keys = Keys::default();
        keys.update(key(4, 8), true);
        keys.update(key(5, 8), true);
        let out = keys.cancel();
        assert_eq!(out.len(), 3);
        assert_eq!(out.last(), Some(&Output::Modifiers(0)));
        assert!(keys.update(key(4, 0), false).is_empty());
        assert!(keys.update(key(5, 0), false).is_empty());
        assert!(matches!(
            keys.update(key(4, 0), true)[0],
            Output::Key { repeat: false, .. }
        ));
    }
    #[test]
    fn ipad_held_bound() {
        let mut keys = Keys::default();
        for hid in 0..64 {
            keys.update(key(hid, 0), true);
        }
        let out = keys.update(key(64, 0), true);
        assert_eq!(out.len(), 65);
        assert!(keys.held.is_empty());
    }
    #[test]
    fn ipad_modifier_release_stale_flags() {
        let mut keys = Keys::default();
        keys.update(key(227, 8), true);
        let up = keys.update(key(227, 8), false);
        assert_eq!(up[0], Output::Modifiers(0));
        assert!(keys.update(key(227, 8), false).is_empty());
    }
    #[test]
    fn ipad_modifier_release_preserves_other_side() {
        let mut keys = Keys::default();
        keys.update(key(227, 8), true);
        keys.update(key(231, 8), true);
        let left_up = keys.update(key(227, 8), false);
        assert!(!left_up.iter().any(|o| matches!(o, Output::Modifiers(0))));
        assert_eq!(keys.update(key(231, 8), false)[0], Output::Modifiers(0));
    }
    #[test]
    fn ipad_named_keys_never_insert_names() {
        for text in ["UIKeyInputLeftArrow", "UIKeyInputEscape", "UIKeyInputHome"] {
            let mut s = key(80, 0);
            s.text = text.into();
            assert_eq!(text_for(&s, true), None);
        }
        assert_eq!(text_for(&key(4, 0), true), Some("a".into()));
        assert_eq!(text_for(&key(4, 8), true), None);
    }
}
