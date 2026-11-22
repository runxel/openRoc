//! The user's names for the profiles.
//!
//! The mouse has no room for names; ROCCAT Swarm keeps them in its own files on
//! the PC as well. openRoc stores them in a small text file, one line per
//! profile slot.

use std::path::PathBuf;

pub struct Names {
	names: Vec<String>,
}

fn file() -> Option<PathBuf> {
	let home = std::env::var_os("HOME")?;
	Some(PathBuf::from(home).join("Library/Application Support/openRoc/profile_names.txt"))
}

impl Names {
	pub fn load() -> Self {
		let text = file()
			.and_then(|path| std::fs::read_to_string(path).ok())
			.unwrap_or_default();
		Self {
			names: text.lines().map(|line| line.trim().to_string()).collect(),
		}
	}

	/// The stored name of a profile slot; empty if it has none.
	pub fn get(&self, index: usize) -> &str {
		self.names.get(index).map_or("", String::as_str)
	}

	/// The name to show: the stored one, or "Profile 3".
	pub fn display(&self, index: usize) -> String {
		match self.get(index) {
			"" => format!("Profile {}", index + 1),
			name => name.to_string(),
		}
	}

	/// Sets a name and saves the file. Returns what went wrong, if anything.
	pub fn set(&mut self, index: usize, name: &str) -> Result<(), String> {
		if self.names.len() <= index {
			self.names.resize(index + 1, String::new());
		}
		self.names[index] = name.chars().filter(|c| !c.is_control()).collect();
		let path = file().ok_or("no home folder")?;
		if let Some(folder) = path.parent() {
			std::fs::create_dir_all(folder).map_err(|e| e.to_string())?;
		}
		let text: String = self.names.iter().map(|name| format!("{name}\n")).collect();
		std::fs::write(path, text).map_err(|e| e.to_string())
	}
}
