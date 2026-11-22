//! openRoc: configure a ROCCAT Kone XP on macOS.

mod buttons;
mod effects;
mod layout;
mod lighting;
mod media;
mod names;
mod theme;

use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::time::{Duration, Instant};

use eframe::egui::{self, Color32, CornerRadius, RichText, Sense, Stroke, StrokeKind, vec2};
use openroc_core::export::{self, ExportedProfile};
use openroc_core::keys::Layout as KeyLayout;
use openroc_core::profile::{
	DPI_MAX, DPI_MIN, DPI_STAGES, DPI_UNIT, INPUT_COUNT, INPUT_NAMES, POLLING_RATES,
};
use openroc_core::{Buttons, Device, DeviceError, Settings};

use crate::media::Media;
use crate::names::Names;

/// The value that follows `flag` on the command line.
fn argument<'a>(args: &'a [String], flag: &str) -> Option<&'a String> {
	args.iter()
		.position(|a| a == flag)
		.and_then(|i| args.get(i + 1))
}

fn main() -> eframe::Result {
	let args: Vec<String> = std::env::args().collect();
	// `--dump` prints the profiles, `--export <file>` saves them; both only read.
	let task = if args.iter().any(|a| a == "--dump") {
		Some(dump())
	} else {
		argument(&args, "--export").map(|path| export_all(path))
	};
	if let Some(result) = task {
		if let Err(e) = result {
			eprintln!("error: {e}");
			std::process::exit(1);
		}
		return Ok(());
	}
	// Start options, mainly for taking screenshots: `--tab <name>` opens that
	// tab, `--input <1-15>` selects an input on the Buttons tab, `--easy-shift`
	// shows the second layer there, `--single-leds` every LED on the Lighting tab.
	let tab = match argument(&args, "--tab").map(String::as_str) {
		Some("dpi") => Tab::Dpi,
		Some("lighting") => Tab::Lighting,
		Some("advanced") => Tab::Advanced,
		_ => Tab::Buttons,
	};
	let input = argument(&args, "--input")
		.and_then(|n| n.parse::<usize>().ok())
		.filter(|n| (1..=INPUT_COUNT).contains(n))
		.map(|n| n - 1);
	let easy_shift = args.iter().any(|a| a == "--easy-shift");
	// `--file <export>` shows the profiles of an export file instead of a mouse's.
	let file = argument(&args, "--file").cloned();
	let single_leds = args.iter().any(|a| a == "--single-leds");
	// `--still <effect number> <seconds>` shows that effect frozen at that time.
	let still = args.iter().position(|a| a == "--still").and_then(|i| {
		let effect = args.get(i + 1)?.parse::<u8>().ok()?;
		let seconds = args.get(i + 2)?.parse::<f64>().ok()?;
		Some((effect, seconds))
	});
	let options = eframe::NativeOptions {
		viewport: egui::ViewportBuilder::default()
			.with_icon(media::app_icon())
			.with_inner_size([1320.0, 840.0])
			.with_min_inner_size([1120.0, 720.0]),
		..Default::default()
	};
	eframe::run_native(
		"openRoc",
		options,
		Box::new(move |cc| {
			theme::apply(&cc.egui_ctx);
			let mut app = App::new(cc.egui_ctx.clone(), tab, input, file.as_deref());
			app.buttons_state.layer = easy_shift as usize;
			app.lighting_state.single_leds = single_leds;
			app.lighting_state.still = still;
			Ok(Box::new(app))
		}),
	)
}

/// Saves all profiles of the mouse to an export file; reads only.
fn export_all(path: &str) -> Result<(), Box<dyn std::error::Error>> {
	let names = Names::load();
	let device = Device::open()?;
	let mut profiles = Vec::new();
	for index in 0..device.profile_info()?.count {
		profiles.push(ExportedProfile {
			name: names.get(index as usize).to_string(),
			settings: device.read_settings(index)?,
			buttons: device.read_buttons(index)?,
		});
	}
	std::fs::write(path, export::to_text(&profiles))?;
	println!("Exported {} profiles to {path}", profiles.len());
	Ok(())
}

/// Prints what is stored on the mouse; reads only.
fn dump() -> Result<(), Box<dyn std::error::Error>> {
	let layout = layout::current();
	let device = Device::open()?;
	let info = device.profile_info()?;
	println!(
		"ROCCAT Kone XP, firmware {}, active profile {} of {}",
		device.firmware_version()?,
		info.active + 1,
		info.count
	);
	for index in 0..info.count {
		let settings = device.read_settings(index)?;
		let buttons = device.read_buttons(index)?;
		let dpi: Vec<String> = (0..DPI_STAGES)
			.map(|s| {
				let mark = if s == settings.active_dpi_stage() {
					"*"
				} else {
					""
				};
				format!("{}{mark}", settings.dpi(s))
			})
			.collect();
		println!("\nProfile {}  DPI {}", index + 1, dpi.join(" / "));
		for (input, name) in INPUT_NAMES.iter().enumerate() {
			println!(
				"  {:<18} {:<22} {}",
				name,
				buttons.slot(input).label(&layout),
				buttons.slot(input + INPUT_COUNT).label(&layout)
			);
		}
	}
	Ok(())
}

#[derive(Clone, PartialEq)]
struct Profile {
	settings: Settings,
	buttons: Buttons,
}

struct Connection {
	active: u8,
	/// What the mouse currently stores.
	stored: Vec<Profile>,
	/// What the user sees and edits.
	edited: Vec<Profile>,
}

#[derive(PartialEq, Clone, Copy)]
enum Tab {
	Buttons,
	Dpi,
	Lighting,
	Advanced,
}

/// Work for the device thread. All HID traffic happens there: on macOS hidapi
/// spins the calling thread's run loop, which must not be the UI thread.
enum Command {
	Connect,
	Write(Vec<Profile>),
	Activate(u8),
}

enum Reply {
	Connected(Result<Snapshot, String>),
	Written {
		stored: Vec<Profile>,
		status: String,
	},
	Activated(Result<u8, String>),
	/// The mouse switched profiles by itself, through one of its buttons.
	ActiveChanged(u8),
}

struct Snapshot {
	active: u8,
	profiles: Vec<Profile>,
}

fn read_all(device: &Device) -> Result<Snapshot, DeviceError> {
	let info = device.profile_info()?;
	let mut profiles = Vec::new();
	for index in 0..info.count {
		profiles.push(Profile {
			settings: device.read_settings(index)?,
			buttons: device.read_buttons(index)?,
		});
	}
	Ok(Snapshot {
		active: info.active,
		profiles,
	})
}

/// Writes every changed report, updating `stored` as far as it gets.
fn write_changes(device: &Device, edited: &[Profile], stored: &mut [Profile]) -> String {
	let mut written = 0;
	for (i, (new, old)) in edited.iter().zip(stored.iter_mut()).enumerate() {
		if new.settings != old.settings {
			if let Err(e) = device.write_settings(&new.settings) {
				return format!("Profile {}: writing settings failed: {e}", i + 1);
			}
			old.settings = new.settings.clone();
			written += 1;
		}
		if new.buttons != old.buttons {
			if let Err(e) = device.write_buttons(&new.buttons) {
				return format!("Profile {}: writing buttons failed: {e}", i + 1);
			}
			old.buttons = new.buttons.clone();
			written += 1;
		}
	}
	format!("Wrote and verified {written} change set(s).")
}

/// How often the device thread asks the mouse which profile is active. A
/// button on the mouse can switch profiles at any time, and the mouse does not
/// have to be asked first.
const ACTIVE_CHECK_INTERVAL: Duration = Duration::from_secs(1);

fn device_thread(commands: Receiver<Command>, replies: Sender<Reply>, ctx: egui::Context) {
	let mut device: Option<Device> = None;
	let mut stored: Vec<Profile> = Vec::new();
	// The active profile as the UI last heard of it.
	let mut active: Option<u8> = None;
	loop {
		let command = match commands.recv_timeout(ACTIVE_CHECK_INTERVAL) {
			Ok(command) => command,
			Err(RecvTimeoutError::Disconnected) => break,
			Err(RecvTimeoutError::Timeout) => {
				// Nothing to do: look whether the mouse switched profiles. A
				// read that fails is skipped; the next command reports the
				// problem.
				let now = device
					.as_ref()
					.and_then(|d| d.profile_info().ok())
					.map(|info| info.active);
				if let Some(now) = now
					&& active != Some(now)
				{
					active = Some(now);
					if replies.send(Reply::ActiveChanged(now)).is_err() {
						break;
					}
					ctx.request_repaint();
				}
				continue;
			}
		};
		let reply = match command {
			Command::Connect => {
				device = None;
				active = None;
				let opened = Device::open().and_then(|d| {
					let snapshot = read_all(&d)?;
					Ok((d, snapshot))
				});
				match opened {
					Ok((d, snapshot)) => {
						device = Some(d);
						active = Some(snapshot.active);
						stored = snapshot.profiles.clone();
						Reply::Connected(Ok(snapshot))
					}
					Err(e) => Reply::Connected(Err(e.to_string())),
				}
			}
			Command::Write(edited) => {
				let status = match &device {
					Some(d) => write_changes(d, &edited, &mut stored),
					None => "Not connected.".to_string(),
				};
				Reply::Written {
					stored: stored.clone(),
					status,
				}
			}
			Command::Activate(index) => {
				let result = match &device {
					Some(d) => d
						.set_active_profile(index)
						.map(|()| index)
						.map_err(|e| e.to_string()),
					None => Err("not connected".to_string()),
				};
				if result.is_ok() {
					active = Some(index);
				}
				Reply::Activated(result)
			}
		};
		if replies.send(reply).is_err() {
			break;
		}
		ctx.request_repaint();
	}
}

struct App {
	commands: Sender<Command>,
	replies: Receiver<Reply>,
	busy: bool,
	/// The profiles on screen come from a file, not from a mouse, so they
	/// cannot be written or made active.
	offline: bool,
	connection: Option<Connection>,
	selected: usize,
	tab: Tab,
	buttons_state: buttons::State,
	lighting_state: lighting::State,
	names: Names,
	status: String,
	media: Media,
	/// Key names for the keyboard layout currently selected in macOS.
	layout: KeyLayout,
	layout_checked: Instant,
}

/// How often the UI asks macOS whether the keyboard layout changed.
const LAYOUT_CHECK_INTERVAL: Duration = Duration::from_secs(1);

impl App {
	fn new(ctx: egui::Context, tab: Tab, input: Option<usize>, file: Option<&str>) -> Self {
		let (commands, command_rx) = channel();
		let (reply_tx, replies) = channel();
		let media = Media::load(&ctx);
		std::thread::spawn(move || device_thread(command_rx, reply_tx, ctx));
		let mut app = Self {
			commands,
			replies,
			busy: false,
			offline: false,
			connection: None,
			selected: 0,
			tab,
			buttons_state: buttons::State { layer: 0, input },
			lighting_state: lighting::State::default(),
			names: Names::load(),
			status: String::new(),
			media,
			layout: layout::current(),
			layout_checked: Instant::now(),
		};
		match file {
			Some(path) => app.show_file(path),
			None => app.request(Command::Connect, "Reading profiles from the mouse…"),
		}
		app
	}

	/// Shows the profiles of an export file without a mouse.
	fn show_file(&mut self, path: &str) {
		let read = std::fs::read_to_string(path)
			.map_err(|e| e.to_string())
			.and_then(|text| export::from_text(&text).map_err(|e| e.to_string()));
		match read {
			Ok(profiles) => {
				let profiles: Vec<Profile> = profiles
					.into_iter()
					.map(|p| Profile {
						settings: p.settings,
						buttons: p.buttons,
					})
					.collect();
				self.connection = Some(Connection {
					active: 0,
					edited: profiles.clone(),
					stored: profiles,
				});
				self.selected = 0;
				self.offline = true;
				let name = std::path::Path::new(path)
					.file_name()
					.map_or(path.into(), |name| name.to_string_lossy());
				self.status = format!("Showing {name}. Nothing here is sent to a mouse.");
			}
			Err(e) => self.status = format!("{path}: {e}"),
		}
	}

	fn request(&mut self, command: Command, status: &str) {
		self.busy = true;
		self.status = status.to_string();
		let _ = self.commands.send(command);
	}

	fn poll(&mut self) {
		while let Ok(reply) = self.replies.try_recv() {
			// Every reply but this one answers a command.
			if !matches!(reply, Reply::ActiveChanged(_)) {
				self.busy = false;
			}
			match reply {
				Reply::Connected(Ok(snapshot)) => {
					self.offline = false;
					self.selected = snapshot.active as usize;
					self.status = "Read all profiles from the mouse.".into();
					self.connection = Some(Connection {
						active: snapshot.active,
						edited: snapshot.profiles.clone(),
						stored: snapshot.profiles,
					});
				}
				Reply::Connected(Err(e)) => {
					self.offline = false;
					self.connection = None;
					self.status = format!("Not connected: {e}");
				}
				Reply::Written { stored, status } => {
					if let Some(c) = &mut self.connection {
						c.stored = stored;
					}
					self.status = status;
				}
				Reply::Activated(Ok(index)) => {
					if let Some(c) = &mut self.connection {
						c.active = index;
					}
					self.status = format!("Profile {} is now active.", index + 1);
				}
				Reply::Activated(Err(e)) => self.status = format!("Switching profile failed: {e}"),
				Reply::ActiveChanged(index) => {
					if let Some(c) = &mut self.connection {
						c.active = index;
					}
					self.status = format!("The mouse switched to profile {}.", index + 1);
				}
			}
		}
	}

	/// Status on the left, the actions on the right.
	fn bottom_bar(&mut self, ui: &mut egui::Ui, dirty: bool) {
		let busy = self.busy || self.offline;
		let reading = self.busy;
		ui.horizontal(|ui| {
			ui.label(theme::dim(self.status.clone()));
			ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
				if theme::primary_button(ui, dirty && !busy, "Write to mouse").clicked() {
					let edited = self.connection.as_ref().map(|c| c.edited.clone());
					if let Some(edited) = edited {
						self.request(Command::Write(edited), "Writing to the mouse…");
					}
				}
				if ui
					.add_enabled(dirty, egui::Button::new("Discard changes"))
					.clicked() && let Some(c) = &mut self.connection
				{
					c.edited = c.stored.clone();
					self.status = "Changes discarded.".into();
				}
				if ui
					.add_enabled(!reading, egui::Button::new("Reload from mouse"))
					.clicked()
				{
					self.request(Command::Connect, "Reading profiles from the mouse…");
				}
			});
		});
	}

	/// Device name, the list of profiles and what can be done with a profile.
	fn sidebar(&mut self, ui: &mut egui::Ui) {
		ui.add_space(4.0);
		ui.heading("Kone XP");
		let Some(c) = &self.connection else {
			ui.label(theme::dim("No mouse found"));
			return;
		};
		ui.add_space(14.0);
		theme::caption(ui, "Profiles");
		let active = c.active as usize;
		let rows: Vec<([u8; 3], bool)> = c
			.edited
			.iter()
			.zip(&c.stored)
			.map(|(edited, stored)| (edited.settings.profile_color(), edited != stored))
			.collect();
		ui.scope(|ui| {
			ui.spacing_mut().item_spacing.y = 3.0;
			for (i, (color, changed)) in rows.into_iter().enumerate() {
				let name = self.names.display(i);
				if profile_row(ui, self.selected == i, &name, color, i == active, changed).clicked()
				{
					self.selected = i;
				}
			}
		});

		ui.add_space(10.0);
		theme::caption(ui, "Name and colour");
		ui.horizontal(|ui| {
			let (rect, _) = ui.allocate_exact_size(vec2(28.0, 28.0), Sense::hover());
			if let Some(c) = &mut self.connection {
				let settings = &mut c.edited[self.selected].settings;
				let before = settings.profile_color();
				let mut rgb = before;
				theme::swatch(ui, rect, &mut rgb, false);
				if rgb != before {
					settings.set_profile_color(rgb);
				}
			}
			let mut name = self.names.get(self.selected).to_string();
			let field = egui::TextEdit::singleline(&mut name)
				.hint_text(format!("Profile {}", self.selected + 1))
				.char_limit(NAME_LIMIT)
				.desired_width(f32::INFINITY);
			if ui.add(field).changed()
				&& let Err(e) = self.names.set(self.selected, &name)
			{
				self.status = format!("The name could not be saved: {e}");
			}
		});
		ui.label(
			RichText::new("The name is kept on this Mac. The mouse stores only the colour.")
				.size(11.5)
				.color(theme::TEXT_DIM),
		);

		ui.add_space(10.0);
		let wide = |label: &str| egui::Button::new(label).min_size(vec2(198.0, 28.0));
		if ui
			.add_enabled(
				self.selected != active && !self.busy && !self.offline,
				wide("Make active"),
			)
			.clicked()
		{
			self.request(Command::Activate(self.selected as u8), "Switching profile…");
		}

		ui.add_space(14.0);
		theme::caption(ui, "Backup");
		if ui.add(wide("Export this profile…")).clicked() {
			self.export(false);
		}
		if ui.add(wide("Export all profiles…")).clicked() {
			self.export(true);
		}
		if ui.add(wide("Import…")).clicked() {
			self.import();
		}
	}

	/// Saves the selected profile, or all of them, to a file the user picks.
	fn export(&mut self, all: bool) {
		let Some(c) = &self.connection else {
			return;
		};
		let range = if all {
			0..c.edited.len()
		} else {
			self.selected..self.selected + 1
		};
		let unwritten = range.clone().any(|i| c.edited[i] != c.stored[i]);
		let profiles: Vec<ExportedProfile> = range
			.map(|i| ExportedProfile {
				name: self.names.get(i).to_string(),
				settings: c.edited[i].settings.clone(),
				buttons: c.edited[i].buttons.clone(),
			})
			.collect();
		let file_name = if all {
			"Kone XP profiles".to_string()
		} else {
			self.names.display(self.selected)
		};
		let Some(path) = rfd::FileDialog::new()
			.set_title("Export profiles")
			.set_file_name(format!("{file_name}.{}", export::FILE_EXTENSION))
			.add_filter("openRoc profiles", &[export::FILE_EXTENSION])
			.save_file()
		else {
			return;
		};
		self.status = match std::fs::write(&path, export::to_text(&profiles)) {
			Ok(()) => format!(
				"Exported {} to {}{}.",
				if all { "all profiles" } else { "the profile" },
				path.display(),
				if unwritten {
					", including changes not yet written to the mouse"
				} else {
					""
				}
			),
			Err(e) => format!("Export failed: {e}"),
		};
	}

	/// Loads profiles from a file into the editor. A file with one profile
	/// replaces the selected profile; a file with several replaces profile 1
	/// onwards. Nothing reaches the mouse until the user writes it.
	fn import(&mut self) {
		let Some(path) = rfd::FileDialog::new()
			.set_title("Import profiles")
			.add_filter("openRoc profiles", &[export::FILE_EXTENSION])
			.pick_file()
		else {
			return;
		};
		let read = std::fs::read_to_string(&path)
			.map_err(|e| e.to_string())
			.and_then(|text| export::from_text(&text).map_err(|e| e.to_string()));
		let profiles = match read {
			Ok(profiles) => profiles,
			Err(e) => {
				self.status = format!("Import failed: {e}");
				return;
			}
		};
		let Some(c) = &mut self.connection else {
			return;
		};
		let first = if profiles.len() == 1 {
			self.selected
		} else {
			0
		};
		let mut last = first;
		for (offset, imported) in profiles.into_iter().enumerate() {
			let slot = first + offset;
			if slot >= c.edited.len() {
				break;
			}
			let ExportedProfile {
				name,
				mut settings,
				mut buttons,
			} = imported;
			settings.set_profile_index(slot as u8);
			buttons.set_profile_index(slot as u8);
			c.edited[slot] = Profile { settings, buttons };
			if !name.is_empty() {
				let _ = self.names.set(slot, &name);
			}
			last = slot;
		}
		let target = if last == first {
			format!("profile {}", first + 1)
		} else {
			format!("profiles {} to {}", first + 1, last + 1)
		};
		self.status = format!("Imported into {target}. Press \"Write to mouse\" to store it.");
	}
}

/// Longest profile name, so that it fits its row in the sidebar.
const NAME_LIMIT: usize = 16;

/// One profile in the sidebar: its colour and name, whether it is the active
/// one, and a dot if it has changes that are not on the mouse yet.
fn profile_row(
	ui: &mut egui::Ui,
	selected: bool,
	name: &str,
	color: [u8; 3],
	active: bool,
	changed: bool,
) -> egui::Response {
	let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 34.0), Sense::click());
	let fill = if selected {
		theme::ACCENT_FILL
	} else if response.hovered() {
		theme::CARD_HOVER
	} else {
		Color32::TRANSPARENT
	};
	let painter = ui.painter();
	painter.rect(
		rect,
		CornerRadius::same(7),
		fill,
		Stroke::NONE,
		StrokeKind::Inside,
	);
	let dot = rect.left_center() + vec2(16.0, 0.0);
	painter.circle_filled(dot, 6.0, Color32::from_rgb(color[0], color[1], color[2]));
	painter.text(
		rect.left_center() + vec2(32.0, 0.0),
		egui::Align2::LEFT_CENTER,
		name,
		egui::FontId::proportional(14.0),
		theme::TEXT,
	);
	let mut right = rect.right_center() - vec2(10.0, 0.0);
	if changed {
		painter.circle_filled(right - vec2(3.0, 0.0), 3.0, theme::WARNING);
		right.x -= 12.0;
	}
	if active {
		painter.text(
			right,
			egui::Align2::RIGHT_CENTER,
			"active",
			egui::FontId::proportional(11.5),
			if selected {
				theme::ACCENT
			} else {
				theme::TEXT_DIM
			},
		);
	}
	if changed {
		response.on_hover_text("This profile has changes that are not on the mouse yet.")
	} else {
		response
	}
}

impl eframe::App for App {
	fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
		self.poll();
		if self.layout_checked.elapsed() >= LAYOUT_CHECK_INTERVAL {
			self.layout = layout::current();
			self.layout_checked = Instant::now();
		}
		let dirty = self
			.connection
			.as_ref()
			.is_some_and(|c| c.edited != c.stored);

		egui::Panel::bottom("actions")
			.frame(
				egui::Frame::new()
					.fill(theme::PANEL)
					.inner_margin(egui::Margin::symmetric(16, 10)),
			)
			.show(ui, |ui| self.bottom_bar(ui, dirty));

		egui::Panel::left("profiles")
			.resizable(false)
			.exact_size(230.0)
			.frame(theme::side_panel())
			.show(ui, |ui| self.sidebar(ui));

		egui::CentralPanel::default()
			.frame(egui::Frame::new().fill(theme::BACKGROUND))
			.show(ui, |ui| {
				let Some(c) = &mut self.connection else {
					ui.centered_and_justified(|ui| {
						ui.label(theme::dim(
							"Connect a ROCCAT Kone XP and press \"Reload from mouse\".",
						));
					});
					return;
				};
				egui::Panel::top("tabs")
					.show_separator_line(false)
					.frame(egui::Frame::new().inner_margin(egui::Margin {
						left: 16,
						right: 16,
						top: 14,
						bottom: 2,
					}))
					.show(ui, |ui| {
						theme::segmented(
							ui,
							&mut self.tab,
							&[
								(Tab::Buttons, "Buttons"),
								(Tab::Dpi, "DPI"),
								(Tab::Lighting, "Lighting"),
								(Tab::Advanced, "Advanced"),
							],
						);
					});
				let profile = &mut c.edited[self.selected];
				let media = &self.media;
				match self.tab {
					Tab::Buttons => buttons::show(
						ui,
						media,
						&mut profile.buttons,
						&mut self.buttons_state,
						&self.layout,
					),
					Tab::Lighting => {
						lighting::show(ui, media, &mut profile.settings, &mut self.lighting_state);
					}
					Tab::Dpi => page(ui, |ui| dpi_tab(ui, &mut profile.settings)),
					Tab::Advanced => page(ui, |ui| advanced_tab(ui, &mut profile.settings)),
				}
			});
	}
}

/// A scrolling page with the usual margins, for tabs made of cards.
fn page(ui: &mut egui::Ui, add_contents: impl FnOnce(&mut egui::Ui)) {
	egui::CentralPanel::default()
		.frame(egui::Frame::new().inner_margin(egui::Margin::same(16)))
		.show(ui, |ui| {
			egui::ScrollArea::vertical()
				.auto_shrink([false, false])
				.show(ui, add_contents);
		});
}

fn dpi_tab(ui: &mut egui::Ui, settings: &mut Settings) {
	theme::card().show(ui, |ui| {
		ui.set_width(560.0);
		ui.label(RichText::new("DPI stages").size(16.0));
		ui.label(theme::dim(
			"The DPI buttons step through the enabled stages. The selected stage is the one the \
			 profile starts with.",
		));
		ui.add_space(8.0);
		egui::Grid::new("dpi")
			.num_columns(4)
			.spacing([18.0, 12.0])
			.show(ui, |ui| {
				for stage in 0..DPI_STAGES {
					if ui
						.radio(
							settings.active_dpi_stage() == stage,
							format!("Stage {}", stage + 1),
						)
						.clicked()
					{
						settings.set_active_dpi_stage(stage);
					}
					let mut dpi = settings.dpi(stage);
					let slider = egui::Slider::new(&mut dpi, DPI_MIN..=DPI_MAX)
						.step_by(DPI_UNIT as f64)
						.logarithmic(true)
						.show_value(false);
					let mut changed = ui.add_sized([260.0, 20.0], slider).changed();
					changed |= ui
						.add(
							egui::DragValue::new(&mut dpi)
								.range(DPI_MIN..=DPI_MAX)
								.speed(DPI_UNIT as f64)
								.suffix(" DPI"),
						)
						.changed();
					if changed {
						settings.set_dpi(stage, dpi);
					}
					let mut enabled = settings.dpi_stage_enabled(stage);
					if ui.checkbox(&mut enabled, "Enabled").changed() {
						settings.set_dpi_stage_enabled(stage, enabled);
					}
					ui.end_row();
				}
			});
		ui.add_space(4.0);
		ui.label(
			RichText::new(format!(
				"{DPI_MIN} to {DPI_MAX} DPI in steps of {DPI_UNIT}. Double-click a value to type it."
			))
			.size(11.5)
			.color(theme::TEXT_DIM),
		);
	});
}

fn advanced_tab(ui: &mut egui::Ui, settings: &mut Settings) {
	theme::card().show(ui, |ui| {
		ui.set_width(560.0);
		ui.label(RichText::new("Sensor and power").size(16.0));
		ui.add_space(8.0);
		egui::Grid::new("advanced")
			.num_columns(2)
			.spacing([24.0, 14.0])
			.show(ui, |ui| {
				ui.label("Polling rate");
				let mut rate = settings.polling_rate_index();
				let options: Vec<(usize, String)> = POLLING_RATES
					.iter()
					.enumerate()
					.map(|(index, hz)| (index, format!("{hz} Hz")))
					.collect();
				let options: Vec<(usize, &str)> = options
					.iter()
					.map(|(index, label)| (*index, label.as_str()))
					.collect();
				theme::segmented(ui, &mut rate, &options);
				if rate != settings.polling_rate_index() && rate < POLLING_RATES.len() {
					settings.set_polling_rate_index(rate);
				}
				ui.end_row();

				ui.label("Angle snapping");
				let mut snapping = settings.angle_snapping();
				if ui
					.checkbox(&mut snapping, "Straighten nearly straight lines")
					.changed()
				{
					settings.set_angle_snapping(snapping);
				}
				ui.end_row();

				ui.label("Lights off after");
				ui.horizontal(|ui| {
					let mut minutes = settings.led_timeout_minutes();
					if ui
						.add(
							egui::DragValue::new(&mut minutes)
								.range(0..=60)
								.suffix(" min"),
						)
						.changed()
					{
						settings.set_led_timeout_minutes(minutes);
					}
					ui.label(theme::dim("without movement"));
				});
				ui.end_row();
			});
		ui.add_space(6.0);
		ui.label(
			RichText::new(
				"Only 1000 Hz has been seen on a real mouse; the other rates follow Swarm's order. \
				 Debounce time and lift-off distance apply to all profiles and cannot be changed \
				 here yet.",
			)
			.size(11.5)
			.color(theme::TEXT_DIM),
		);
	});
}
