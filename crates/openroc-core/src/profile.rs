//! The two per-profile reports: settings (0x06) and buttons (0x07).
//!
//! Both types keep the raw report and only touch the bytes whose meaning is
//! known, so everything not yet understood is written back unchanged.

use crate::keys::Layout;

pub const REPORT_SETTINGS: u8 = 0x06;
pub const REPORT_BUTTONS: u8 = 0x07;
pub const SETTINGS_LEN: usize = 174;
pub const BUTTONS_LEN: usize = 125;

pub const DPI_STAGES: usize = 5;
pub const DPI_UNIT: u16 = 50;
pub const DPI_MIN: u16 = 50;
pub const DPI_MAX: u16 = 19_000;
pub const LED_COUNT: usize = 20;

/// Light stripes on each side of the mouse. Every stripe has two LEDs.
pub const STRIPES_PER_SIDE: usize = 4;
/// Single LEDs that shine faintly into all four stripes of their side.
pub const LED_FILL_LEFT: usize = 8;
pub const LED_FILL_RIGHT: usize = 9;
pub const LED_WHEEL: usize = 18;
/// The button behind the wheel.
pub const LED_BUTTON: usize = 19;

/// The LED at one end of a light stripe, as tested on a device. `stripe` counts
/// from left to right within its side; `rear` selects the palm end instead of
/// the cable end.
pub const fn stripe_led(right_side: bool, stripe: usize, rear: bool) -> usize {
	assert!(stripe < STRIPES_PER_SIDE);
	(if right_side { 10 } else { 0 }) + stripe * 2 + rear as usize
}

/// Polling rates in Hz, indexed by the stored value.
pub const POLLING_RATES: [u16; 4] = [125, 250, 500, 1000];
pub const EFFECT_STATIC: u8 = 0x01;
pub const EFFECT_BLINKING: u8 = 0x02;
pub const EFFECT_BREATHING: u8 = 0x03;
pub const EFFECT_HEARTBEAT: u8 = 0x04;
pub const EFFECT_PHOTON: u8 = 0x05;
pub const EFFECT_WAVE: u8 = 0x09;
pub const EFFECT_AIMO: u8 = 0x0a;

/// Lighting effects as (stored number, name), in the order of Swarm's list.
/// Each number was set on a mouse (firmware 1.09) and the result observed.
/// Numbers 6 to 8 are accepted too but look the same as static.
pub const LIGHT_EFFECTS: &[(u8, &str)] = &[
	(EFFECT_STATIC, "Static colours"),
	(EFFECT_BLINKING, "Blinking"),
	(EFFECT_BREATHING, "Breathing"),
	(EFFECT_HEARTBEAT, "Heartbeat"),
	(EFFECT_WAVE, "Wave"),
	(EFFECT_PHOTON, "Photon FX"),
	(EFFECT_AIMO, "AIMO intelligent lighting"),
];

/// The effect speeds Swarm offers. The mouse accepts larger numbers, but they
/// run no faster than the highest of these.
pub const LIGHT_SPEED_MIN: u8 = 1;
pub const LIGHT_SPEED_MAX: u8 = 11;

/// Physical inputs per layer; the Easy-Shift layer repeats them.
pub const INPUT_COUNT: usize = 15;
pub const SLOT_COUNT: usize = INPUT_COUNT * 2;

/// Slot order as tested on the device.
pub const INPUT_NAMES: [&str; INPUT_COUNT] = [
	"Left button",
	"Right button",
	"Wheel click",
	"Wheel tilt left",
	"Wheel tilt right",
	"Wheel up",
	"Wheel down",
	"\"+\" button",
	"\"−\" button",
	"Thumb T1",
	"Thumb T2",
	"Thumb T3",
	"Thumb T4",
	"Easy-Shift button",
	"Behind the wheel",
];

const OFF_INDEX: usize = 0x02;
const OFF_DPI_MASK: usize = 0x05;
const OFF_DPI_ACTIVE: usize = 0x06;
const OFF_DPI: usize = 0x07;
const OFF_ANGLE_SNAPPING: usize = 0x1b;
const OFF_POLLING: usize = 0x1d;
const OFF_LIGHT_EFFECT: usize = 0x1e;
const OFF_LIGHT_SPEED: usize = 0x1f;
const OFF_LIGHT_BRIGHTNESS: usize = 0x20;
const OFF_LED_TIMEOUT: usize = 0x21;
const OFF_LEDS: usize = 0x24;
const LED_ENTRY: usize = 6;
const OFF_PROFILE_COLOR: usize = 0x9f;
const OFF_SLOTS: usize = 0x03;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ProfileError {
	#[error("expected {expected} bytes, got {got}")]
	Length { expected: usize, got: usize },
	#[error("unexpected report header {0:02x} {1:02x}")]
	Header(u8, u8),
	#[error("checksum mismatch: stored {stored:#06x}, computed {computed:#06x}")]
	Checksum { stored: u16, computed: u16 },
}

/// 16-bit sum of all bytes, as used at the end of reports 0x06, 0x07 and 0x08.
pub fn checksum(data: &[u8]) -> u16 {
	data.iter().fold(0u16, |sum, b| sum.wrapping_add(*b as u16))
}

fn validate<const N: usize>(bytes: &[u8], report_id: u8) -> Result<[u8; N], ProfileError> {
	let raw: [u8; N] = bytes.try_into().map_err(|_| ProfileError::Length {
		expected: N,
		got: bytes.len(),
	})?;
	if raw[0] != report_id || raw[1] as usize != N {
		return Err(ProfileError::Header(raw[0], raw[1]));
	}
	let stored = u16::from_le_bytes([raw[N - 2], raw[N - 1]]);
	let computed = checksum(&raw[..N - 2]);
	if stored != computed {
		return Err(ProfileError::Checksum { stored, computed });
	}
	Ok(raw)
}

fn seal<const N: usize>(raw: &[u8; N]) -> [u8; N] {
	let mut out = *raw;
	let sum = checksum(&out[..N - 2]).to_le_bytes();
	out[N - 2] = sum[0];
	out[N - 1] = sum[1];
	out
}

/// Report 0x06: DPI, lighting and sensor settings of one profile.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Settings {
	raw: [u8; SETTINGS_LEN],
}

impl Settings {
	pub fn from_bytes(bytes: &[u8]) -> Result<Self, ProfileError> {
		Ok(Self {
			raw: validate(bytes, REPORT_SETTINGS)?,
		})
	}

	/// The report with a fresh checksum, ready to send.
	pub fn to_bytes(&self) -> [u8; SETTINGS_LEN] {
		seal(&self.raw)
	}

	pub fn profile_index(&self) -> u8 {
		self.raw[OFF_INDEX]
	}

	/// Moves the settings to another profile slot.
	pub fn set_profile_index(&mut self, index: u8) {
		self.raw[OFF_INDEX] = index;
	}

	pub fn dpi_stage_enabled(&self, stage: usize) -> bool {
		self.raw[OFF_DPI_MASK] & (1 << stage) != 0
	}

	pub fn set_dpi_stage_enabled(&mut self, stage: usize, enabled: bool) {
		assert!(stage < DPI_STAGES);
		if enabled {
			self.raw[OFF_DPI_MASK] |= 1 << stage;
		} else {
			self.raw[OFF_DPI_MASK] &= !(1 << stage);
		}
	}

	pub fn active_dpi_stage(&self) -> usize {
		self.raw[OFF_DPI_ACTIVE] as usize
	}

	pub fn set_active_dpi_stage(&mut self, stage: usize) {
		assert!(stage < DPI_STAGES);
		self.raw[OFF_DPI_ACTIVE] = stage as u8;
	}

	/// DPI of a stage in real DPI (the report stores units of 50).
	pub fn dpi(&self, stage: usize) -> u16 {
		let o = OFF_DPI + stage * 2;
		u16::from_le_bytes([self.raw[o], self.raw[o + 1]]).saturating_mul(DPI_UNIT)
	}

	/// Sets a stage, clamped to the sensor range and rounded to 50 DPI.
	pub fn set_dpi(&mut self, stage: usize, dpi: u16) {
		assert!(stage < DPI_STAGES);
		let units = (dpi.clamp(DPI_MIN, DPI_MAX) + DPI_UNIT / 2) / DPI_UNIT;
		let o = OFF_DPI + stage * 2;
		self.raw[o..o + 2].copy_from_slice(&units.to_le_bytes());
	}

	pub fn angle_snapping(&self) -> bool {
		self.raw[OFF_ANGLE_SNAPPING] != 0
	}

	pub fn set_angle_snapping(&mut self, on: bool) {
		self.raw[OFF_ANGLE_SNAPPING] = on as u8;
	}

	/// Index into [`POLLING_RATES`]. Only 3 (1000 Hz) has been seen on a device.
	pub fn polling_rate_index(&self) -> usize {
		self.raw[OFF_POLLING] as usize
	}

	pub fn set_polling_rate_index(&mut self, index: usize) {
		assert!(index < POLLING_RATES.len());
		self.raw[OFF_POLLING] = index as u8;
	}

	/// Raw speed of animated lighting effects.
	pub fn light_speed(&self) -> u8 {
		self.raw[OFF_LIGHT_SPEED]
	}

	pub fn set_light_speed(&mut self, speed: u8) {
		self.raw[OFF_LIGHT_SPEED] = speed;
	}

	/// 0–255, where 255 is full brightness.
	pub fn light_brightness(&self) -> u8 {
		self.raw[OFF_LIGHT_BRIGHTNESS]
	}

	pub fn set_light_brightness(&mut self, brightness: u8) {
		self.raw[OFF_LIGHT_BRIGHTNESS] = brightness;
	}

	/// Minutes of inactivity before the LEDs switch off.
	pub fn led_timeout_minutes(&self) -> u8 {
		self.raw[OFF_LED_TIMEOUT]
	}

	pub fn set_led_timeout_minutes(&mut self, minutes: u8) {
		self.raw[OFF_LED_TIMEOUT] = minutes;
	}

	/// Lighting effect number, see [`LIGHT_EFFECTS`].
	pub fn light_effect(&self) -> u8 {
		self.raw[OFF_LIGHT_EFFECT]
	}

	pub fn set_light_effect(&mut self, effect: u8) {
		self.raw[OFF_LIGHT_EFFECT] = effect;
	}

	pub fn led_color(&self, led: usize) -> [u8; 3] {
		let o = OFF_LEDS + led * LED_ENTRY + 2;
		[self.raw[o], self.raw[o + 1], self.raw[o + 2]]
	}

	pub fn set_led_color(&mut self, led: usize, rgb: [u8; 3]) {
		assert!(led < LED_COUNT);
		let o = OFF_LEDS + led * LED_ENTRY + 2;
		self.raw[o..o + 3].copy_from_slice(&rgb);
	}

	/// The profile's colour as shown in Swarm's profile slots.
	pub fn profile_color(&self) -> [u8; 3] {
		let o = OFF_PROFILE_COLOR;
		[self.raw[o], self.raw[o + 1], self.raw[o + 2]]
	}

	pub fn set_profile_color(&mut self, rgb: [u8; 3]) {
		self.raw[OFF_PROFILE_COLOR..OFF_PROFILE_COLOR + 3].copy_from_slice(&rgb);
	}
}

/// Report 0x07: the 30 button slots of one profile.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Buttons {
	raw: [u8; BUTTONS_LEN],
}

impl Buttons {
	pub fn from_bytes(bytes: &[u8]) -> Result<Self, ProfileError> {
		Ok(Self {
			raw: validate(bytes, REPORT_BUTTONS)?,
		})
	}

	pub fn to_bytes(&self) -> [u8; BUTTONS_LEN] {
		seal(&self.raw)
	}

	pub fn profile_index(&self) -> u8 {
		self.raw[OFF_INDEX]
	}

	/// Moves the buttons to another profile slot.
	pub fn set_profile_index(&mut self, index: u8) {
		self.raw[OFF_INDEX] = index;
	}

	/// Slots 0–14 are the normal layer, 15–29 the Easy-Shift layer.
	pub fn slot(&self, slot: usize) -> Binding {
		let o = OFF_SLOTS + slot * 4;
		Binding([
			self.raw[o],
			self.raw[o + 1],
			self.raw[o + 2],
			self.raw[o + 3],
		])
	}

	pub fn set_slot(&mut self, slot: usize, binding: Binding) {
		assert!(slot < SLOT_COUNT);
		let o = OFF_SLOTS + slot * 4;
		self.raw[o..o + 4].copy_from_slice(&binding.0);
	}

	/// True if some normal-layer slot still produces a left click.
	pub fn has_left_click(&self) -> bool {
		(0..INPUT_COUNT).any(|s| self.slot(s) == Binding::LEFT_CLICK)
	}
}

pub const GROUP_MOUSE: u8 = 0x01;
pub const GROUP_KEY: u8 = 0x06;

/// One named function of the mouse. The codes come from the menus of ROCCAT
/// Swarm's Kone XP module. `verified` means the name was also matched against
/// what Swarm shows for bytes stored on a real mouse; otherwise it was assigned
/// by menu position only.
pub struct KnownFunction {
	pub binding: Binding,
	pub name: &'static str,
	pub verified: bool,
}

const fn known(group: u8, function: u8, name: &'static str, verified: bool) -> KnownFunction {
	KnownFunction {
		binding: Binding::function(group, function),
		name,
		verified,
	}
}

pub const KNOWN_FUNCTIONS: &[KnownFunction] = &[
	known(0x00, 0x00, "Disabled", true),
	known(0x01, 0x01, "Left click", true),
	known(0x01, 0x02, "Right click", true),
	known(0x01, 0x03, "Middle click", true),
	known(0x01, 0x04, "Double click", false),
	known(0x01, 0x05, "Browser forward", true),
	known(0x01, 0x06, "Browser back", true),
	known(0x01, 0x07, "Tilt left", true),
	known(0x01, 0x08, "Tilt right", true),
	known(0x01, 0x09, "Scroll up", true),
	known(0x01, 0x0a, "Scroll down", true),
	known(0x02, 0x01, "Cycle DPI", false),
	known(0x02, 0x02, "DPI up", true),
	known(0x02, 0x03, "DPI down", true),
	known(0x03, 0x04, "Play / pause", true),
	known(0x03, 0x05, "Stop playback", false),
	known(0x03, 0x02, "Previous track", true),
	known(0x03, 0x03, "Next track", true),
	known(0x03, 0x06, "Mute", true),
	known(0x03, 0x07, "Volume up", true),
	known(0x03, 0x08, "Volume down", true),
	known(0x03, 0x0b, "Browser search", false),
	known(0x03, 0x0c, "Browser stop", false),
	known(0x03, 0x0d, "Browser reload", false),
	known(0x03, 0x0e, "Browser new tab", false),
	known(0x03, 0x0f, "Browser new window", false),
	known(0x04, 0x08, "Left Shift (hold)", true),
	known(0x08, 0x01, "Cycle profiles", true),
	known(0x08, 0x02, "Next profile", false),
	known(0x08, 0x03, "Previous profile", false),
	known(0x08, 0x0b, "Lighting on/off", true),
	known(0x0a, 0x01, "Easy-Shift", true),
];

/// The four bytes of a button slot: `[0, key, function-or-modifiers, group]`.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct Binding(pub [u8; 4]);

impl Binding {
	pub const LEFT_CLICK: Binding = Binding::function(GROUP_MOUSE, 0x01);
	/// The button that switches to the second layer while it is held.
	pub const EASY_SHIFT: Binding = Binding::function(0x0a, 0x01);

	pub const fn function(group: u8, function: u8) -> Self {
		Binding([0, 0, function, group])
	}

	/// A keyboard key (HID usage) with a modifier mask from [`keys`].
	pub const fn key(usage: u8, modifiers: u8) -> Self {
		Binding([0, usage, modifiers, GROUP_KEY])
	}

	pub fn group(&self) -> u8 {
		self.0[3]
	}

	pub fn is_key(&self) -> bool {
		self.group() == GROUP_KEY
	}

	pub fn key_usage(&self) -> u8 {
		self.0[1]
	}

	pub fn key_modifiers(&self) -> u8 {
		self.0[2]
	}

	pub fn known(&self) -> Option<&'static KnownFunction> {
		KNOWN_FUNCTIONS.iter().find(|k| k.binding == *self)
	}

	/// Human-readable name, with keys named for `layout`; unknown functions
	/// show their raw numbers.
	pub fn label(&self, layout: &Layout) -> String {
		if self.is_key() {
			return layout.shortcut_label(self.key_usage(), self.key_modifiers());
		}
		match self.known() {
			Some(k) => k.name.to_string(),
			None => format!("Function {:02x}/{:02x}", self.0[2], self.0[3]),
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::keys;

	const P0_SETTINGS: &[u8] = include_bytes!("../tests/data/profile0_settings.bin");
	const P0_BUTTONS: &[u8] = include_bytes!("../tests/data/profile0_buttons.bin");
	const P4_SETTINGS: &[u8] = include_bytes!("../tests/data/profile4_settings.bin");
	const P4_BUTTONS: &[u8] = include_bytes!("../tests/data/profile4_buttons.bin");

	#[test]
	fn dumps_round_trip_unchanged() {
		for dump in [P0_SETTINGS, P4_SETTINGS] {
			assert_eq!(Settings::from_bytes(dump).unwrap().to_bytes(), dump);
		}
		for dump in [P0_BUTTONS, P4_BUTTONS] {
			assert_eq!(Buttons::from_bytes(dump).unwrap().to_bytes(), dump);
		}
	}

	#[test]
	fn rejects_corrupt_reports() {
		let mut bad = P0_SETTINGS.to_vec();
		bad[10] ^= 0xff;
		assert!(matches!(
			Settings::from_bytes(&bad),
			Err(ProfileError::Checksum { .. })
		));
		assert!(matches!(
			Settings::from_bytes(P0_BUTTONS),
			Err(ProfileError::Length { .. })
		));
		assert!(matches!(
			Buttons::from_bytes(&P0_SETTINGS[..BUTTONS_LEN]),
			Err(ProfileError::Header(..))
		));
	}

	#[test]
	fn decodes_factory_settings() {
		let s = Settings::from_bytes(P4_SETTINGS).unwrap();
		assert_eq!(s.profile_index(), 4);
		let dpi: Vec<u16> = (0..DPI_STAGES).map(|i| s.dpi(i)).collect();
		assert_eq!(dpi, [400, 800, 1200, 1600, 3200]);
		assert!((0..DPI_STAGES).all(|i| s.dpi_stage_enabled(i)));
		assert_eq!(s.active_dpi_stage(), 4);
		assert_eq!(s.light_effect(), EFFECT_AIMO);
		assert_eq!(POLLING_RATES[s.polling_rate_index()], 1000);
		assert!(!s.angle_snapping());
		assert_eq!(s.led_timeout_minutes(), 15);
		assert_eq!(s.light_brightness(), 0xff);
		assert_eq!(s.profile_color(), [0xf6, 0xff, 0x39]);
		assert_eq!(s.led_color(0), [0x00, 0x48, 0xff]);
		assert_eq!(s.led_color(LED_COUNT - 1), [0x00, 0x48, 0xff]);
	}

	#[test]
	fn moving_to_another_slot_keeps_the_report_valid() {
		let mut s = Settings::from_bytes(P4_SETTINGS).unwrap();
		let mut b = Buttons::from_bytes(P4_BUTTONS).unwrap();
		s.set_profile_index(1);
		b.set_profile_index(1);
		let s = Settings::from_bytes(&s.to_bytes()).unwrap();
		let b = Buttons::from_bytes(&b.to_bytes()).unwrap();
		assert_eq!((s.profile_index(), b.profile_index()), (1, 1));
		assert_eq!(s.dpi(4), 3200);
		assert_eq!(b.slot(13), Binding::EASY_SHIFT);
	}

	#[test]
	fn led_positions_cover_every_entry_once() {
		let mut leds = vec![LED_FILL_LEFT, LED_FILL_RIGHT, LED_WHEEL, LED_BUTTON];
		for right_side in [false, true] {
			for stripe in 0..STRIPES_PER_SIDE {
				leds.push(stripe_led(right_side, stripe, false));
				leds.push(stripe_led(right_side, stripe, true));
			}
		}
		leds.sort();
		assert_eq!(leds, (0..LED_COUNT).collect::<Vec<_>>());
		assert_eq!(stripe_led(false, 0, true), 1);
		assert_eq!(stripe_led(true, 3, false), 16);
	}

	#[test]
	fn dpi_edit_changes_only_dpi_and_checksum() {
		let mut s = Settings::from_bytes(P4_SETTINGS).unwrap();
		s.set_dpi(2, 1234);
		assert_eq!(s.dpi(2), 1250);
		s.set_dpi(0, 60_000);
		assert_eq!(s.dpi(0), DPI_MAX);
		s.set_dpi(1, 0);
		assert_eq!(s.dpi(1), DPI_MIN);

		let out = s.to_bytes();
		let changed: Vec<usize> = (0..SETTINGS_LEN)
			.filter(|&i| out[i] != P4_SETTINGS[i])
			.collect();
		assert!(
			changed
				.iter()
				.all(|&i| (0x07..0x0d).contains(&i) || i >= SETTINGS_LEN - 2)
		);
		assert!(Settings::from_bytes(&out).is_ok());
	}

	#[test]
	fn decodes_factory_and_custom_buttons() {
		let us = Layout::us();
		let b = Buttons::from_bytes(P4_BUTTONS).unwrap();
		assert_eq!(b.slot(0), Binding::LEFT_CLICK);
		assert_eq!(b.slot(7).label(&us), "DPI up");
		assert_eq!(b.slot(13).label(&us), "Easy-Shift");
		assert_eq!(b.slot(11), Binding::key(0x15, 0));
		assert!(b.has_left_click());

		let custom = Buttons::from_bytes(P0_BUTTONS).unwrap();
		assert_eq!(custom.slot(9).label(&us), "Enter");
		assert_eq!(custom.slot(12).label(&us), "Cmd+Y");
		let german = Layout::new("German", [(0x1c, "Z".to_string())]);
		assert_eq!(custom.slot(12).label(&german), "Cmd+Z");
		assert_eq!(custom.slot(23).label(&us), "Shift+Cmd+T");
		assert_eq!(custom.slot(11).label(&us), "Left Shift (hold)");
		assert_eq!(custom.slot(17).label(&us), "Mute");
		assert_eq!(custom.slot(15).label(&us), "Browser back");
		assert!((0..SLOT_COUNT).all(|i| !custom.slot(i).label(&us).starts_with("Function")));
		assert!((0..SLOT_COUNT).all(|i| !b.slot(i).label(&us).starts_with("Function")));
	}

	#[test]
	fn slot_edit_round_trips() {
		let mut b = Buttons::from_bytes(P4_BUTTONS).unwrap();
		b.set_slot(29, Binding::key(0x04, keys::MOD_SHIFT));
		let again = Buttons::from_bytes(&b.to_bytes()).unwrap();
		assert_eq!(again.slot(29).label(&Layout::us()), "Shift+A");
		assert_eq!(again.slot(28), b.slot(28));
		b.set_slot(0, Binding::function(0, 0));
		assert!(!b.has_left_click());
	}
}
