//! Keyboard keys (USB HID usage page 0x07) and the modifier mask used in key bindings.

use std::collections::HashMap;

pub const MOD_CTRL: u8 = 0x01;
pub const MOD_SHIFT: u8 = 0x02;
pub const MOD_ALT: u8 = 0x04;
pub const MOD_CMD: u8 = 0x08;

/// Modifier bits in display order. Shift and Cmd are confirmed on the device;
/// Ctrl and Option follow the standard HID order.
pub const MODIFIERS: [(u8, &str); 4] = [
	(MOD_CTRL, "Ctrl"),
	(MOD_ALT, "Option"),
	(MOD_SHIFT, "Shift"),
	(MOD_CMD, "Cmd"),
];

const USAGE_LAST_LETTER: u8 = 0x1d;
/// The extra key next to left Shift on ISO keyboards ("<" on a German one).
pub const USAGE_ISO_KEY: u8 = 0x64;

/// Keys offered in the editor, as (HID usage, US name). Letters come first.
pub const KEYS: &[(u8, &str)] = &[
	(0x04, "A"),
	(0x05, "B"),
	(0x06, "C"),
	(0x07, "D"),
	(0x08, "E"),
	(0x09, "F"),
	(0x0a, "G"),
	(0x0b, "H"),
	(0x0c, "I"),
	(0x0d, "J"),
	(0x0e, "K"),
	(0x0f, "L"),
	(0x10, "M"),
	(0x11, "N"),
	(0x12, "O"),
	(0x13, "P"),
	(0x14, "Q"),
	(0x15, "R"),
	(0x16, "S"),
	(0x17, "T"),
	(0x18, "U"),
	(0x19, "V"),
	(0x1a, "W"),
	(0x1b, "X"),
	(0x1c, "Y"),
	(0x1d, "Z"),
	(0x1e, "1"),
	(0x1f, "2"),
	(0x20, "3"),
	(0x21, "4"),
	(0x22, "5"),
	(0x23, "6"),
	(0x24, "7"),
	(0x25, "8"),
	(0x26, "9"),
	(0x27, "0"),
	(0x28, "Enter"),
	(0x29, "Escape"),
	(0x2a, "Backspace"),
	(0x2b, "Tab"),
	(0x2c, "Space"),
	(0x2d, "-"),
	(0x2e, "="),
	(0x2f, "["),
	(0x30, "]"),
	(0x31, "\\"),
	(0x33, ";"),
	(0x34, "'"),
	(0x35, "`"),
	(0x36, ","),
	(0x37, "."),
	(0x38, "/"),
	(USAGE_ISO_KEY, "ISO key"),
	(0x3a, "F1"),
	(0x3b, "F2"),
	(0x3c, "F3"),
	(0x3d, "F4"),
	(0x3e, "F5"),
	(0x3f, "F6"),
	(0x40, "F7"),
	(0x41, "F8"),
	(0x42, "F9"),
	(0x43, "F10"),
	(0x44, "F11"),
	(0x45, "F12"),
	(0x4a, "Home"),
	(0x4b, "Page Up"),
	(0x4c, "Delete"),
	(0x4d, "End"),
	(0x4e, "Page Down"),
	(0x4f, "Right"),
	(0x50, "Left"),
	(0x51, "Down"),
	(0x52, "Up"),
];

/// The name of a key position on a US keyboard.
fn us_key_name(usage: u8) -> String {
	match KEYS.iter().find(|(u, _)| *u == usage) {
		Some((_, name)) => name.to_string(),
		None => format!("Key {usage:02x}"),
	}
}

/// True for positions whose character depends on the keyboard layout.
pub fn is_layout_dependent(usage: u8) -> bool {
	matches!(usage, 0x04..=0x27 | 0x2d..=0x38 | USAGE_ISO_KEY)
}

/// Names for key positions under one keyboard layout.
///
/// The mouse stores positions (HID usages), not characters: usage `0x1c` types
/// "Y" on a US keyboard and "Z" on a German one. A layout maps positions to the
/// characters the computer currently produces for them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Layout {
	/// Display name such as "German"; empty for the built-in US names.
	pub name: String,
	names: HashMap<u8, String>,
}

impl Layout {
	/// The built-in US names.
	pub fn us() -> Self {
		Self::default()
	}

	/// A layout that names the given positions; all others keep their US name.
	pub fn new(name: impl Into<String>, names: impl IntoIterator<Item = (u8, String)>) -> Self {
		Self {
			name: name.into(),
			names: names.into_iter().collect(),
		}
	}

	pub fn key_name(&self, usage: u8) -> String {
		match self.names.get(&usage) {
			Some(name) => name.clone(),
			None => us_key_name(usage),
		}
	}

	pub fn shortcut_label(&self, usage: u8, modifiers: u8) -> String {
		let mut parts: Vec<String> = MODIFIERS
			.iter()
			.filter(|(bit, _)| modifiers & bit != 0)
			.map(|(_, name)| name.to_string())
			.collect();
		let unknown = modifiers & !(MOD_CTRL | MOD_SHIFT | MOD_ALT | MOD_CMD);
		if unknown != 0 {
			parts.push(format!("Mod {unknown:02x}"));
		}
		parts.push(self.key_name(usage));
		parts.join("+")
	}

	/// The keys of [`KEYS`] named for this layout, letters in alphabetical order.
	pub fn keys(&self) -> Vec<(u8, String)> {
		let mut keys: Vec<(u8, String)> = KEYS
			.iter()
			.map(|(usage, _)| (*usage, self.key_name(*usage)))
			.collect();
		let letters = KEYS
			.iter()
			.take_while(|(usage, _)| *usage <= USAGE_LAST_LETTER)
			.count();
		keys[..letters].sort_by(|a, b| a.1.cmp(&b.1));
		keys
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn us_layout_uses_builtin_names() {
		let us = Layout::us();
		assert_eq!(us.key_name(0x1c), "Y");
		assert_eq!(us.key_name(0xee), "Key ee");
		assert_eq!(us.shortcut_label(0x17, MOD_SHIFT | MOD_CMD), "Shift+Cmd+T");
	}

	#[test]
	fn other_layout_renames_and_sorts_letters() {
		let german = Layout::new(
			"German",
			[
				(0x1c, "Z".to_string()),
				(0x1d, "Y".to_string()),
				(0x2f, "Ü".to_string()),
			],
		);
		assert_eq!(german.shortcut_label(0x1c, MOD_CMD), "Cmd+Z");
		assert_eq!(german.key_name(0x28), "Enter");
		let keys = german.keys();
		assert_eq!(keys.len(), KEYS.len());
		assert_eq!(keys[24], (0x1d, "Y".to_string()));
		assert_eq!(keys[25], (0x1c, "Z".to_string()));
		assert!(keys.contains(&(0x2f, "Ü".to_string())));
	}
}
