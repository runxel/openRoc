//! Profiles as a text file, for backups and for moving a profile to another
//! slot or mouse.
//!
//! ```text
//! openRoc profiles 1
//! device: ROCCAT Kone XP
//!
//! [profile]
//! name: Work
//! settings: 06ae00…
//! buttons: 077d00…
//! ```
//!
//! `settings` and `buttons` are the raw reports `0x06` and `0x07` in hex, so a
//! file keeps everything the mouse stores for a profile, including the parts
//! openRoc does not understand yet. Macros are not included.

use crate::profile::{Buttons, ProfileError, Settings};

const MAGIC: &str = "openRoc profiles";
const VERSION: &str = "1";
pub const DEVICE_NAME: &str = "ROCCAT Kone XP";
/// File name extension for exported profiles, without the dot.
pub const FILE_EXTENSION: &str = "openroc";

/// One profile as stored in an export file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExportedProfile {
	/// The user's name for the profile; empty if it has none.
	pub name: String,
	pub settings: Settings,
	pub buttons: Buttons,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ExportError {
	#[error("this is not an openRoc profile file")]
	NotAnExport,
	#[error("the file uses format version {0}, which this version of openRoc cannot read")]
	Version(String),
	#[error("the file was made for another device ({0})")]
	OtherDevice(String),
	#[error("line {0} is not understood")]
	Line(usize),
	#[error("profile {profile} has no {field}")]
	Missing { profile: usize, field: &'static str },
	#[error("profile {profile}: the {field} are damaged: {source}")]
	Report {
		profile: usize,
		field: &'static str,
		source: ProfileError,
	},
	#[error("the file contains no profiles")]
	Empty,
}

fn hex(bytes: &[u8]) -> String {
	bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn unhex(text: &str) -> Option<Vec<u8>> {
	let text = text.trim();
	if !text.is_ascii() || !text.len().is_multiple_of(2) {
		return None;
	}
	(0..text.len())
		.step_by(2)
		.map(|i| u8::from_str_radix(&text[i..i + 2], 16).ok())
		.collect()
}

/// Writes the profiles as an export file.
pub fn to_text(profiles: &[ExportedProfile]) -> String {
	let mut text = format!("{MAGIC} {VERSION}\ndevice: {DEVICE_NAME}\n");
	for profile in profiles {
		let name: String = profile.name.chars().filter(|c| !c.is_control()).collect();
		text.push_str(&format!(
			"\n[profile]\nname: {}\nsettings: {}\nbuttons: {}\n",
			name.trim(),
			hex(&profile.settings.to_bytes()),
			hex(&profile.buttons.to_bytes())
		));
	}
	text
}

#[derive(Default)]
struct Draft {
	name: String,
	settings: Option<Vec<u8>>,
	buttons: Option<Vec<u8>>,
}

fn finish(draft: Draft, number: usize) -> Result<ExportedProfile, ExportError> {
	let settings = draft.settings.ok_or(ExportError::Missing {
		profile: number,
		field: "settings",
	})?;
	let buttons = draft.buttons.ok_or(ExportError::Missing {
		profile: number,
		field: "buttons",
	})?;
	Ok(ExportedProfile {
		name: draft.name,
		settings: Settings::from_bytes(&settings).map_err(|source| ExportError::Report {
			profile: number,
			field: "settings",
			source,
		})?,
		buttons: Buttons::from_bytes(&buttons).map_err(|source| ExportError::Report {
			profile: number,
			field: "buttons",
			source,
		})?,
	})
}

/// Reads an export file. Every report is checked for length and checksum.
pub fn from_text(text: &str) -> Result<Vec<ExportedProfile>, ExportError> {
	let mut lines = text
		.lines()
		.enumerate()
		.map(|(index, line)| (index + 1, line.trim()))
		.filter(|(_, line)| !line.is_empty());
	let (_, first) = lines.next().ok_or(ExportError::NotAnExport)?;
	let version = first
		.strip_prefix(MAGIC)
		.ok_or(ExportError::NotAnExport)?
		.trim();
	if version != VERSION {
		return Err(ExportError::Version(version.to_string()));
	}

	let mut profiles = Vec::new();
	let mut draft: Option<Draft> = None;
	for (number, line) in lines {
		if line == "[profile]" {
			if let Some(done) = draft.take() {
				profiles.push(finish(done, profiles.len() + 1)?);
			}
			draft = Some(Draft::default());
			continue;
		}
		let (key, value) = line.split_once(':').ok_or(ExportError::Line(number))?;
		let value = value.trim();
		match (key.trim(), &mut draft) {
			("device", None) if value == DEVICE_NAME => {}
			("device", None) => return Err(ExportError::OtherDevice(value.to_string())),
			("name", Some(draft)) => draft.name = value.to_string(),
			("settings", Some(draft)) => {
				draft.settings = Some(unhex(value).ok_or(ExportError::Line(number))?);
			}
			("buttons", Some(draft)) => {
				draft.buttons = Some(unhex(value).ok_or(ExportError::Line(number))?);
			}
			// Unknown keys are skipped so that later versions can add fields.
			_ => {}
		}
	}
	if let Some(done) = draft {
		profiles.push(finish(done, profiles.len() + 1)?);
	}
	if profiles.is_empty() {
		return Err(ExportError::Empty);
	}
	Ok(profiles)
}

#[cfg(test)]
mod tests {
	use super::*;

	fn sample(name: &str, settings: &[u8], buttons: &[u8]) -> ExportedProfile {
		ExportedProfile {
			name: name.to_string(),
			settings: Settings::from_bytes(settings).unwrap(),
			buttons: Buttons::from_bytes(buttons).unwrap(),
		}
	}

	fn samples() -> Vec<ExportedProfile> {
		vec![
			sample(
				"LX-Mac",
				include_bytes!("../tests/data/profile0_settings.bin"),
				include_bytes!("../tests/data/profile0_buttons.bin"),
			),
			sample(
				"",
				include_bytes!("../tests/data/profile4_settings.bin"),
				include_bytes!("../tests/data/profile4_buttons.bin"),
			),
		]
	}

	#[test]
	fn round_trips() {
		let profiles = samples();
		let text = to_text(&profiles);
		assert!(text.starts_with("openRoc profiles 1\n"));
		assert_eq!(from_text(&text).unwrap(), profiles);
		// Windows line endings and extra fields are accepted.
		let windows = text
			.replace('\n', "\r\n")
			.replace("[profile]", "[profile]\r\nnote: x");
		assert_eq!(from_text(&windows).unwrap(), profiles);
	}

	#[test]
	fn rejects_other_files() {
		assert_eq!(from_text(""), Err(ExportError::NotAnExport));
		assert_eq!(from_text("hello\nworld"), Err(ExportError::NotAnExport));
		assert_eq!(
			from_text("openRoc profiles 2\n"),
			Err(ExportError::Version("2".to_string()))
		);
		assert_eq!(
			from_text("openRoc profiles 1\ndevice: ROCCAT Kone XP\n"),
			Err(ExportError::Empty)
		);
		assert_eq!(
			from_text("openRoc profiles 1\ndevice: Other Mouse\n"),
			Err(ExportError::OtherDevice("Other Mouse".to_string()))
		);
	}

	#[test]
	fn rejects_damaged_profiles() {
		let text = to_text(&samples());
		let missing = text.replace("buttons: ", "knobs: ");
		assert_eq!(
			from_text(&missing),
			Err(ExportError::Missing {
				profile: 1,
				field: "buttons"
			})
		);
		// Flip one hex digit inside the first settings report.
		let at = text.find("settings: ").unwrap() + 30;
		let mut damaged = text.clone();
		let digit = if &text[at..at + 1] == "0" { "1" } else { "0" };
		damaged.replace_range(at..at + 1, digit);
		assert!(matches!(
			from_text(&damaged),
			Err(ExportError::Report {
				profile: 1,
				field: "settings",
				..
			})
		));
	}
}
