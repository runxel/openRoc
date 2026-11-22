//! How the lighting effects look over time, for the preview.
//!
//! The mouse runs its effects itself. ROCCAT Swarm shows them with a
//! simulation of its own, read from Swarm's `aimo.dll`. This module takes the
//! heartbeat's shape, the rainbow and the frames of Photon FX from there.
//! Swarm's pace does not match the mouse, though, and its wave moves
//! differently: how long a cycle takes and how the wave moves was observed on
//! a real mouse with a stopwatch. See "Lighting effects over time" in
//! `docs/PROTOCOL.md`. The real mouse may differ in detail.

use openroc_core::Settings;
use openroc_core::profile::{
	EFFECT_BLINKING, EFFECT_BREATHING, EFFECT_HEARTBEAT, EFFECT_PHOTON, EFFECT_STATIC, EFFECT_WAVE,
	LED_BUTTON, LED_COUNT, LED_FILL_LEFT, LED_FILL_RIGHT, LED_WHEEL, LIGHT_SPEED_MAX,
	LIGHT_SPEED_MIN, STRIPES_PER_SIDE, stripe_led,
};

pub type Colours = [[u8; 3]; LED_COUNT];

/// True for the effects [`frame`] can animate.
pub fn is_animated(effect: u8) -> bool {
	matches!(
		effect,
		EFFECT_BLINKING | EFFECT_BREATHING | EFFECT_HEARTBEAT | EFFECT_WAVE | EFFECT_PHOTON
	)
}

/// True for the effects that bring their own colours and ignore the stored ones.
pub fn has_own_colours(effect: u8) -> bool {
	matches!(effect, EFFECT_WAVE | EFFECT_PHOTON)
}

/// False for the effects known to look the same at every speed.
pub fn uses_speed(effect: u8) -> bool {
	!matches!(effect, EFFECT_STATIC | EFFECT_WAVE)
}

/// The colour of every LED, `seconds` after the effect started, before the
/// brightness setting is applied. Effects that cannot be animated give the
/// stored colours.
pub fn frame(settings: &Settings, effect: u8, seconds: f64) -> Colours {
	let stored: Colours = std::array::from_fn(|led| settings.led_color(led));
	let speed = settings
		.light_speed()
		.clamp(LIGHT_SPEED_MIN, LIGHT_SPEED_MAX) as f64;
	let seconds = seconds.max(0.0);
	// Where in a cycle of `period` seconds the effect is, from 0 to 1.
	let position = |period: f64| seconds % period / period;
	match effect {
		EFFECT_BLINKING => {
			let on = position(blinking_period(speed)) < 0.5;
			scaled(stored, if on { 1.0 } else { 0.0 })
		}
		EFFECT_BREATHING => {
			let period = breathing_period(speed);
			scaled(stored, breathing_level(seconds % period, period))
		}
		EFFECT_HEARTBEAT => {
			let step = position(heartbeat_period(speed)) * HEARTBEAT_CYCLE as f64;
			scaled(stored, heartbeat_level(step as u64))
		}
		EFFECT_WAVE => wave(position(WAVE_PERIOD)),
		EFFECT_PHOTON => photon(photon_frame(position(photon_period(speed)))),
		_ => stored,
	}
}

fn scaled(mut colours: Colours, level: f32) -> Colours {
	for channel in colours.iter_mut().flatten() {
		*channel = (*channel as f32 * level).round() as u8;
	}
	colours
}

/// Turns a position in a cycle (0 to 1) into a way there and back: 0 at the
/// start, 1 in the middle, 0 again at the end.
fn there_and_back(position: f64) -> f64 {
	if position < 0.5 {
		2.0 * position
	} else {
		2.0 - 2.0 * position
	}
}

// Seconds per cycle on the mouse (firmware 1.09), as straight lines through
// stopwatch timings. They are good to a few tenths of a second.

/// Blinking: 2 s at speed 1, 1 s at speed 10; half of it on, half off.
fn blinking_period(speed: f64) -> f64 {
	2.11 - 0.11 * speed
}

/// Breathing: 8 s at speed 1, 6 s at speed 5, 3.5 s at speed 10, 3 s at the top.
fn breathing_period(speed: f64) -> f64 {
	8.5 - 0.5 * speed
}

/// Heartbeat: 8 s at speed 1, 4.5 s at speed 10, 3.8 s at the top.
fn heartbeat_period(speed: f64) -> f64 {
	8.4 - 0.41 * speed
}

/// Photon FX, in and out again: 8 s at speed 1, 6.8 s at speed 6, 5.1 s at
/// speed 11.
fn photon_period(speed: f64) -> f64 {
	8.37 - 0.29 * speed
}

/// The mouse stays dark for about this long between two breaths, at every
/// speed.
const BREATH_PAUSE: f64 = 0.45;

/// Brightness `seconds` into a breath that takes `period` seconds: up, down,
/// then the dark pause.
fn breathing_level(seconds: f64, period: f64) -> f32 {
	let ramp = (period - BREATH_PAUSE) / 2.0;
	let level = if seconds < ramp {
		seconds / ramp
	} else {
		2.0 - seconds / ramp
	};
	level.clamp(0.0, 1.0) as f32
}

/// A heartbeat in Swarm's steps. The level changes by 1/50 of the full
/// brightness per step: up to full, down to 40 %, up to 75 %, down to dark,
/// then a pause of 11 steps.
const HEARTBEAT_CYCLE: u64 = 147;

fn heartbeat_level(step: u64) -> f32 {
	let n = (step % HEARTBEAT_CYCLE) as f32;
	let level = match n as u64 {
		0..50 => (n + 1.0) / 50.0,
		50..80 => 1.0 - (n - 49.0) / 50.0,
		80..98 => (0.4 + (n - 79.0) / 50.0).min(0.75),
		98..136 => 0.75 - (n - 97.0) / 50.0,
		_ => 0.0,
	};
	level.clamp(0.0, 1.0)
}

/// The mouse as Swarm's effects see it: eleven columns from left to right.
/// Columns 0–3 are the left light stripes, 4 the left fill light, 5 the wheel
/// and the button behind it, 6 the right fill light, 7–10 the right stripes.
const COLUMNS: usize = 11;

fn column(led: usize) -> usize {
	for stripe in 0..STRIPES_PER_SIDE {
		for rear in [false, true] {
			if led == stripe_led(false, stripe, rear) {
				return stripe;
			}
			if led == stripe_led(true, stripe, rear) {
				return 7 + stripe;
			}
		}
	}
	match led {
		LED_FILL_LEFT => 4,
		LED_FILL_RIGHT => 6,
		_ => 5,
	}
}

const RAINBOW_COLOURS: u64 = 384;

/// The colour wheel in 384 steps, from red over yellow, green, cyan, blue and
/// magenta back to red.
fn rainbow(index: u64) -> [u8; 3] {
	let index = index % RAINBOW_COLOURS;
	let k = index % 64;
	let up = (4 * k).min(255) as u8;
	let down = (256 - 4 * k).min(255) as u8;
	match index / 64 {
		0 => [255, up, 0],
		1 => [down, 255, 0],
		2 => [0, 255, up],
		3 => [0, down, 255],
		4 => [up, 0, 255],
		_ => [255, 0, down],
	}
}

// The wave as seen on the mouse: the rainbow comes in from the right and
// moves to the left until the left side is pink, then it runs back. The
// speed setting does not change it.

/// Seconds for the wave to go there and back.
const WAVE_PERIOD: f64 = 11.5;
/// The left edge starts at orange and turns round at pink (magenta).
const WAVE_START: f64 = 32.0;
const WAVE_END: f64 = 320.0;
/// How far the right edge is ahead of the left one in the rainbow: yellow
/// while the left is orange.
const WAVE_SPREAD: f64 = 32.0;

fn wave(position: f64) -> Colours {
	let left = WAVE_START + (WAVE_END - WAVE_START) * there_and_back(position);
	std::array::from_fn(|led| {
		let ahead = WAVE_SPREAD * column(led) as f64 / (COLUMNS - 1) as f64;
		rainbow((left + ahead).round() as u64)
	})
}

/// Photon FX is a pulse of light that runs through the stripes from the
/// outside in, on both sides at once, and changes colour on the way. It
/// visits one stripe end after the other: these are the colours of the eight
/// stops and the frame of Swarm's sequence at which each one reaches full
/// brightness.
const PHOTON_STOPS: [([u8; 3], i64); 8] = [
	([0x9b, 0x08, 0xed], 25),
	([0x15, 0x0b, 0xea], 39),
	([0x08, 0xe1, 0xed], 61),
	([0x0c, 0xe9, 0xaf], 75),
	([0x0f, 0xed, 0x15], 97),
	([0xe7, 0xed, 0x08], 111),
	([0xed, 0x8b, 0x08], 133),
	([0xed, 0x00, 0x00], 147),
];
/// A stop stays at full brightness for this many frames.
const PHOTON_HOLD: i64 = 13;
/// The pulse runs between these two frames, in and out again. At the first
/// one the outermost stop is in the middle of its bright time, so the light
/// never goes out at the turn; Swarm's own preview runs on into the dark
/// frames before it, the mouse does not.
const PHOTON_FIRST: u64 = 31;
const PHOTON_LAST: u64 = 150;
/// Brightest channel of the wheel and button during Photon FX.
const PHOTON_CENTRE_LEVEL: f32 = 100.0;

/// The frame shown at a position in the cycle (0 to 1).
fn photon_frame(position: f64) -> u64 {
	let frames = (PHOTON_LAST - PHOTON_FIRST) as f64;
	PHOTON_FIRST + (there_and_back(position) * frames).round() as u64
}

/// Brightness of a stop: full while the pulse is on it, fading out over about
/// 19 frames before and after.
fn photon_level(frame: u64, full_at: i64) -> f32 {
	let frame = frame as i64;
	let distance = (full_at - frame).max(frame - (full_at + PHOTON_HOLD - 1));
	if distance <= 0 {
		1.0
	} else {
		((0.86f32.powi(distance as i32) - 0.05) / 0.95).max(0.0)
	}
}

fn photon(frame: u64) -> Colours {
	let mut colours = [[0; 3]; LED_COUNT];
	let mut sum = [0.0f32; 3];
	for (stop, (colour, full_at)) in PHOTON_STOPS.iter().enumerate() {
		let level = photon_level(frame, *full_at);
		let lit = colour.map(|c| (c as f32 * level).round() as u8);
		for channel in 0..3 {
			sum[channel] += colour[channel] as f32 * level;
		}
		// The path goes up one stripe and down the next: palm end first on
		// the first and third stripe, cable end first on the others.
		let stripe = stop / 2;
		let rear = (stop % 2 == 0) == (stripe % 2 == 0);
		colours[stripe_led(false, stripe, rear)] = lit;
		colours[stripe_led(true, STRIPES_PER_SIDE - 1 - stripe, rear)] = lit;
	}
	// The wheel and the button show the colour of the pulse, dimmed.
	let brightest = sum.into_iter().fold(0.0, f32::max);
	if brightest > 0.5 {
		let centre = sum.map(|c| (c / brightest * PHOTON_CENTRE_LEVEL).round() as u8);
		colours[LED_WHEEL] = centre;
		colours[LED_BUTTON] = centre;
	}
	colours
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn breathing_goes_up_and_down_and_pauses() {
		// A breath of 4.45 s: 2 s up, 2 s down, 0.45 s dark.
		let level = |seconds| breathing_level(seconds, 4.45);
		assert_eq!(level(0.0), 0.0);
		assert_eq!(level(1.0), 0.5);
		assert_eq!(level(2.0), 1.0);
		assert_eq!(level(3.0), 0.5);
		assert_eq!(level(4.0), 0.0);
		assert_eq!(level(4.4), 0.0);
	}

	#[test]
	fn cycle_times_match_the_stopwatch() {
		let close = |got: f64, measured: f64| {
			assert!(
				(got - measured).abs() <= 0.25,
				"{got} s, measured {measured} s"
			);
		};
		close(blinking_period(1.0), 2.0);
		close(blinking_period(10.0), 1.0);
		close(blinking_period(11.0), 1.0);
		close(breathing_period(1.0), 8.0);
		close(breathing_period(5.0), 6.0);
		close(breathing_period(10.0), 3.5);
		close(breathing_period(11.0), 3.0);
		close(heartbeat_period(1.0), 8.0);
		close(heartbeat_period(10.0), 4.5);
		close(heartbeat_period(11.0), 3.8);
		close(photon_period(1.0), 8.0);
		close(photon_period(6.0), 6.8);
		close(photon_period(11.0), 5.1);
	}

	#[test]
	fn heartbeat_has_two_beats_and_a_pause() {
		let level = |step| (heartbeat_level(step) * 100.0).round() as i32;
		assert_eq!(level(49), 100);
		assert_eq!(level(79), 40);
		assert_eq!(level(97), 75);
		assert_eq!(level(135), 0);
		assert_eq!(level(146), 0);
		assert_eq!(level(147), 2);
		// No jumps: one step never changes the level by more than 1/50.
		for step in 0..2 * HEARTBEAT_CYCLE {
			let change = (heartbeat_level(step + 1) - heartbeat_level(step)).abs();
			assert!(change < 0.021, "step {step}: {change}");
		}
	}

	#[test]
	fn rainbow_matches_swarm_palette() {
		assert_eq!(rainbow(0), [0xff, 0x00, 0x00]);
		assert_eq!(rainbow(1), [0xff, 0x04, 0x00]);
		assert_eq!(rainbow(63), [0xff, 0xfc, 0x00]);
		assert_eq!(rainbow(64), [0xff, 0xff, 0x00]);
		assert_eq!(rainbow(65), [0xfc, 0xff, 0x00]);
		assert_eq!(rainbow(384), rainbow(0));
	}

	#[test]
	fn every_led_has_a_column() {
		let mut count = [0; COLUMNS];
		for led in 0..LED_COUNT {
			count[column(led)] += 1;
		}
		assert_eq!(count, [2, 2, 2, 2, 1, 2, 1, 2, 2, 2, 2]);
	}

	#[test]
	fn wave_runs_to_pink_and_back() {
		let leftmost = stripe_led(false, 0, false);
		let rightmost = stripe_led(true, 3, false);
		// Start: orange on the left, yellow on the right.
		let frame = wave(0.0);
		assert_eq!(frame[leftmost], [0xff, 0x80, 0x00]);
		assert_eq!(frame[rightmost], [0xff, 0xff, 0x00]);
		// A little later green has come in from the right.
		let frame = wave(0.125);
		assert_eq!(frame[leftmost], rainbow(104));
		assert_eq!(frame[rightmost], rainbow(136));
		// Turn: pink on the left, the right already going red.
		let frame = wave(0.5);
		assert_eq!(frame[leftmost], [0xff, 0x00, 0xff]);
		assert_eq!(frame[rightmost], [0xff, 0x00, 0x80]);
		// The way back mirrors the way there.
		assert_eq!(wave(0.875), wave(0.125));
	}

	#[test]
	fn photon_runs_in_and_out_without_going_dark() {
		assert_eq!(photon_frame(0.0), 31);
		assert_eq!(photon_frame(0.5), 150);
		assert_eq!(photon_frame(0.25), photon_frame(0.75));
		assert_eq!(photon_frame(0.999), 31);
		for step in 0..1000 {
			let frame = photon(photon_frame(step as f64 / 1000.0));
			let brightest = frame.iter().flatten().max().unwrap();
			assert!(*brightest > 80, "dark at {step}: {brightest}");
		}
	}

	/// Frames taken from Swarm's table: the colours of the first, second and
	/// seventh stop and of the wheel.
	#[test]
	fn photon_matches_swarm_frames() {
		let first = stripe_led(false, 0, true);
		let second = stripe_led(false, 0, false);
		let seventh = stripe_led(false, 3, false);
		let close = |got: [u8; 3], want: [u8; 3], tolerance: i32| {
			let off = (0..3)
				.map(|c| (got[c] as i32 - want[c] as i32).abs())
				.max()
				.unwrap();
			assert!(off <= tolerance, "{got:?} is not close to {want:?}");
		};
		let frame = photon(0);
		assert!(frame.iter().all(|led| *led == [0, 0, 0]));
		let frame = photon(30);
		close(frame[first], [0x9b, 0x08, 0xed], 0);
		close(frame[second], [0x04, 0x02, 0x2e], 8);
		close(frame[LED_WHEEL], [56, 3, 100], 12);
		let frame = photon(48);
		close(frame[first], [0x12, 0x01, 0x1d], 8);
		close(frame[second], [0x15, 0x0b, 0xea], 0);
		let frame = photon(144);
		close(frame[seventh], [0xed, 0x8b, 0x08], 0);
		close(frame[LED_WHEEL], [100, 36, 2], 12);
		// Both sides mirror each other and the fill lights stay off.
		assert_eq!(frame[stripe_led(true, 0, false)], frame[seventh]);
		assert_eq!(frame[LED_FILL_LEFT], [0, 0, 0]);
		assert_eq!(frame[LED_FILL_RIGHT], [0, 0, 0]);
	}
}
