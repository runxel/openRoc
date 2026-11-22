//! Names the keys of the keyboard layout that is currently selected in macOS.
//!
//! The mouse stores key positions (HID usages). To show a binding the way the
//! user will experience it, each position is translated to the character the
//! current layout produces for it, so usage `0x1c` reads "Z" on a German
//! layout and "Y" on a US one.

use openroc_core::keys::Layout;

/// The layout selected in the menu bar, or the US names if it cannot be read.
/// Call on the main thread only: the macOS text input API requires it.
pub fn current() -> Layout {
	#[cfg(target_os = "macos")]
	if let Some(layout) = macos::current() {
		return layout;
	}
	Layout::us()
}

#[cfg(target_os = "macos")]
mod macos {
	use std::ffi::{CStr, c_char, c_long, c_ulong, c_void};

	use openroc_core::keys::{Layout, USAGE_ISO_KEY};

	type CFTypeRef = *const c_void;

	#[link(name = "Carbon", kind = "framework")]
	unsafe extern "C" {
		static kTISPropertyUnicodeKeyLayoutData: CFTypeRef;
		static kTISPropertyLocalizedName: CFTypeRef;
		fn TISCopyCurrentKeyboardLayoutInputSource() -> CFTypeRef;
		fn TISGetInputSourceProperty(source: CFTypeRef, key: CFTypeRef) -> CFTypeRef;
		fn LMGetKbdType() -> u8;
		fn UCKeyTranslate(
			layout: *const c_void,
			virtual_key: u16,
			action: u16,
			modifiers: u32,
			keyboard_type: u32,
			options: u32,
			dead_key_state: *mut u32,
			max_length: c_ulong,
			actual_length: *mut c_ulong,
			output: *mut u16,
		) -> i32;
	}

	#[link(name = "CoreFoundation", kind = "framework")]
	unsafe extern "C" {
		fn CFRelease(object: CFTypeRef);
		fn CFDataGetBytePtr(data: CFTypeRef) -> *const u8;
		fn CFStringGetCString(
			string: CFTypeRef,
			buffer: *mut c_char,
			size: c_long,
			encoding: u32,
		) -> u8;
	}

	const UC_KEY_ACTION_DISPLAY: u16 = 3;
	const UC_KEY_TRANSLATE_NO_DEAD_KEYS: u32 = 1;
	const CF_STRING_ENCODING_UTF8: u32 = 0x0800_0100;

	/// HID usage → macOS virtual key code, for the keys whose character
	/// depends on the layout.
	const VIRTUAL_KEYS: &[(u8, u16)] = &[
		(0x04, 0x00), // A
		(0x05, 0x0b), // B
		(0x06, 0x08), // C
		(0x07, 0x02), // D
		(0x08, 0x0e), // E
		(0x09, 0x03), // F
		(0x0a, 0x05), // G
		(0x0b, 0x04), // H
		(0x0c, 0x22), // I
		(0x0d, 0x26), // J
		(0x0e, 0x28), // K
		(0x0f, 0x25), // L
		(0x10, 0x2e), // M
		(0x11, 0x2d), // N
		(0x12, 0x1f), // O
		(0x13, 0x23), // P
		(0x14, 0x0c), // Q
		(0x15, 0x0f), // R
		(0x16, 0x01), // S
		(0x17, 0x11), // T
		(0x18, 0x20), // U
		(0x19, 0x09), // V
		(0x1a, 0x0d), // W
		(0x1b, 0x07), // X
		(0x1c, 0x10), // Y
		(0x1d, 0x06), // Z
		(0x1e, 0x12), // 1
		(0x1f, 0x13), // 2
		(0x20, 0x14), // 3
		(0x21, 0x15), // 4
		(0x22, 0x17), // 5
		(0x23, 0x16), // 6
		(0x24, 0x1a), // 7
		(0x25, 0x1c), // 8
		(0x26, 0x19), // 9
		(0x27, 0x1d), // 0
		(0x2d, 0x1b), // -
		(0x2e, 0x18), // =
		(0x2f, 0x21), // [
		(0x30, 0x1e), // ]
		(0x31, 0x2a), // \
		(0x33, 0x29), // ;
		(0x34, 0x27), // '
		(0x35, 0x32), // `
		(0x36, 0x2b), // ,
		(0x37, 0x2f), // .
		(0x38, 0x2c), // /
		(USAGE_ISO_KEY, 0x0a),
	];

	pub fn current() -> Option<Layout> {
		// SAFETY: plain C calls on the main thread. `source` is owned by us and
		// released below; the properties and the layout data it hands out stay
		// valid until then.
		unsafe {
			let source = TISCopyCurrentKeyboardLayoutInputSource();
			if source.is_null() {
				return None;
			}
			let layout = read(source);
			CFRelease(source);
			layout
		}
	}

	unsafe fn read(source: CFTypeRef) -> Option<Layout> {
		unsafe {
			let data = TISGetInputSourceProperty(source, kTISPropertyUnicodeKeyLayoutData);
			if data.is_null() {
				return None;
			}
			let keyboard_layout = CFDataGetBytePtr(data) as *const c_void;
			if keyboard_layout.is_null() {
				return None;
			}
			let keyboard_type = LMGetKbdType() as u32;
			let names = VIRTUAL_KEYS.iter().filter_map(|&(usage, virtual_key)| {
				let mut dead_keys = 0u32;
				let mut length: c_ulong = 0;
				let mut output = [0u16; 4];
				let status = UCKeyTranslate(
					keyboard_layout,
					virtual_key,
					UC_KEY_ACTION_DISPLAY,
					0,
					keyboard_type,
					UC_KEY_TRANSLATE_NO_DEAD_KEYS,
					&mut dead_keys,
					output.len() as c_ulong,
					&mut length,
					output.as_mut_ptr(),
				);
				if status != 0 || length == 0 {
					return None;
				}
				let text = String::from_utf16(&output[..length as usize]).ok()?;
				display_name(&text).map(|name| (usage, name))
			});
			let names: Vec<(u8, String)> = names.collect();
			Some(Layout::new(layout_name(source), names))
		}
	}

	/// Upper-cases a key's character the way key caps show it. Characters
	/// without a single upper-case form (such as "ß") stay as they are.
	fn display_name(text: &str) -> Option<String> {
		let mut chars = text.chars();
		let first = chars.next()?;
		if chars.next().is_some() || first.is_control() || first.is_whitespace() {
			return None;
		}
		let mut upper = first.to_uppercase();
		match (upper.next(), upper.next()) {
			(Some(single), None) => Some(single.to_string()),
			_ => Some(first.to_string()),
		}
	}

	unsafe fn layout_name(source: CFTypeRef) -> String {
		unsafe {
			let name = TISGetInputSourceProperty(source, kTISPropertyLocalizedName);
			let mut buffer = [0 as c_char; 128];
			if name.is_null()
				|| CFStringGetCString(
					name,
					buffer.as_mut_ptr(),
					buffer.len() as c_long,
					CF_STRING_ENCODING_UTF8,
				) == 0
			{
				return "current".to_string();
			}
			CStr::from_ptr(buffer.as_ptr())
				.to_string_lossy()
				.into_owned()
		}
	}
}
