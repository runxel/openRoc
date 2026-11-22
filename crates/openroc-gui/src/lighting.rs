//! The Lighting tab: the mouse with its lights, a colour button for every
//! light connected to its place on the mouse, and the effect controls.

use eframe::egui::{self, Color32, Pos2, Rect, Sense, Shape, Stroke, pos2, vec2};
use openroc_core::Settings;
use openroc_core::profile::{
	EFFECT_AIMO, EFFECT_STATIC, LED_BUTTON, LED_COUNT, LED_FILL_LEFT, LED_FILL_RIGHT, LED_WHEEL,
	LIGHT_EFFECTS, LIGHT_SPEED_MAX, LIGHT_SPEED_MIN, STRIPES_PER_SIDE, stripe_led,
};

use crate::effects;
use crate::media::{MOUSE_SIZE, Media};
use crate::theme;

/// What the Lighting tab remembers between frames.
pub struct State {
	/// Show a colour button for every LED instead of one per zone.
	pub single_leds: bool,
	/// Play the effect in the preview instead of showing the stored colours.
	pub animate: bool,
	/// Show this effect at this time (seconds) instead of the profile's own,
	/// without moving. Set from the command line to take pictures.
	pub still: Option<(u8, f64)>,
}

impl Default for State {
	fn default() -> Self {
		Self {
			single_leds: false,
			animate: true,
			still: None,
		}
	}
}

const SWATCH_SIZE: egui::Vec2 = vec2(40.0, 24.0);
/// Distance between the picture and the columns of colour buttons.
const SWATCH_GAP: f32 = 40.0;

/// Where the light of each stripe LED shows on the picture, per stripe from
/// left to right, in picture pixels.
const LEFT_CABLE: [(f32, f32); 4] = [
	(111.0, 245.0),
	(122.0, 240.0),
	(132.0, 231.0),
	(144.0, 224.0),
];
const LEFT_PALM: [(f32, f32); 4] = [
	(102.0, 350.0),
	(108.0, 366.0),
	(117.0, 378.0),
	(128.0, 384.0),
];
const RIGHT_CABLE: [(f32, f32); 4] = [
	(206.0, 229.0),
	(217.0, 236.0),
	(228.0, 250.0),
	(241.0, 262.0),
];
const RIGHT_PALM: [(f32, f32); 4] = [
	(216.0, 372.0),
	(226.0, 360.0),
	(238.0, 348.0),
	(246.0, 326.0),
];
const WHEEL: (f32, f32) = (177.0, 150.0);
const BUTTON: (f32, f32) = (175.0, 261.0);
const FILL_LEFT: (f32, f32) = (150.0, 300.0);
const FILL_RIGHT: (f32, f32) = (200.0, 300.0);

/// One colour button next to the picture.
struct Callout {
	/// The LED entries it sets.
	leds: Vec<usize>,
	/// Left (false) or right (true) of the picture.
	right_side: bool,
	/// Height of the button, in picture pixels from the top.
	y: f32,
	/// Where its lights are on the picture.
	dots: Vec<(f32, f32)>,
}

/// The colour buttons to show. Within each group the order is chosen so that
/// the lines to the picture do not cross.
fn callouts(single_leds: bool) -> Vec<Callout> {
	let one = |led: usize, right_side: bool, y: f32, dot: (f32, f32)| Callout {
		leds: vec![led],
		right_side,
		y,
		dots: vec![dot],
	};
	let mut list = vec![
		one(LED_WHEEL, true, 70.0, WHEEL),
		one(LED_BUTTON, true, 258.0, BUTTON),
		one(LED_FILL_LEFT, false, 300.0, FILL_LEFT),
		one(LED_FILL_RIGHT, true, 300.0, FILL_RIGHT),
	];
	// (right side, palm end, dots, height of the first button, stripe of the first button)
	let groups = [
		(false, false, LEFT_CABLE, 100.0, 3),
		(false, true, LEFT_PALM, 400.0, 0),
		(true, false, RIGHT_CABLE, 110.0, 0),
		(true, true, RIGHT_PALM, 400.0, 3),
	];
	for (right_side, rear, dots, top, first) in groups {
		if single_leds {
			for row in 0..STRIPES_PER_SIDE {
				let stripe = if first == 0 { row } else { first - row };
				list.push(one(
					stripe_led(right_side, stripe, rear),
					right_side,
					top + row as f32 * 30.0,
					dots[stripe],
				));
			}
		} else {
			list.push(Callout {
				leds: (0..STRIPES_PER_SIDE)
					.map(|stripe| stripe_led(right_side, stripe, rear))
					.collect(),
				right_side,
				y: top + 45.0,
				dots: dots.to_vec(),
			});
		}
	}
	list
}

pub fn show(ui: &mut egui::Ui, media: &Media, settings: &mut Settings, state: &mut State) {
	egui::Panel::right("light-controls")
		.resizable(false)
		.exact_size(290.0)
		.frame(theme::side_panel())
		.show(ui, |ui| controls(ui, settings));

	egui::CentralPanel::default()
		.frame(egui::Frame::new().inner_margin(egui::Margin::same(16)))
		.show(ui, |ui| {
			ui.horizontal(|ui| {
				// Zones or single LEDs only matters while there are colour
				// buttons to show.
				let colours = !effects::has_own_colours(settings.light_effect());
				ui.add_enabled_ui(colours, |ui| {
					theme::segmented(
						ui,
						&mut state.single_leds,
						&[(false, "Zones"), (true, "Single LEDs")],
					);
				});
				ui.add_space(8.0);
				ui.add_enabled(
					effects::is_animated(settings.light_effect()),
					egui::Checkbox::new(&mut state.animate, "Play effect"),
				)
				.on_disabled_hover_text("This effect does not move, so there is nothing to play.")
				.on_hover_text("Show the effect moving. Switch it off to see the stored colours.");
			});
			ui.add_space(8.0);
			preview(ui, media, settings, state);
		});
}

/// Sets all `leds` through one colour button at `rect`. The button shows as
/// mixed if the LEDs differ. Returns the button's response.
fn colour_button(
	ui: &mut egui::Ui,
	rect: Rect,
	settings: &mut Settings,
	leds: &[usize],
) -> egui::Response {
	let before = settings.led_color(leds[0]);
	let mixed = leds.iter().any(|led| settings.led_color(*led) != before);
	let mut rgb = before;
	let response = theme::swatch(ui, rect, &mut rgb, mixed);
	if rgb != before {
		for led in leds {
			settings.set_led_color(*led, rgb);
		}
	}
	response
}

/// The mouse with its lights as the effect shows them, and the colour buttons
/// around it. Effects that bring their own colours get no buttons.
fn preview(ui: &mut egui::Ui, media: &Media, settings: &mut Settings, state: &State) {
	let single_leds = state.single_leds;
	let note_height = 36.0;
	let area = ui.available_rect_before_wrap();
	ui.allocate_rect(area, Sense::hover());

	let space = vec2(
		area.width() - 2.0 * (SWATCH_SIZE.x + SWATCH_GAP + 4.0),
		area.height() - note_height,
	);
	let scale = (space.x / MOUSE_SIZE[0])
		.min(space.y / MOUSE_SIZE[1])
		.clamp(0.4, 1.4);
	let picture = Rect::from_center_size(
		pos2(area.center().x, area.top() + space.y / 2.0),
		vec2(MOUSE_SIZE[0], MOUSE_SIZE[1]) * scale,
	);
	let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
	ui.painter()
		.image(media.mouse.id(), picture, uv, Color32::WHITE);

	let (effect, time) = state
		.still
		.unwrap_or_else(|| (settings.light_effect(), ui.input(|i| i.time)));
	let playing = state.animate && effects::is_animated(effect);
	let shown = if playing { effect } else { EFFECT_STATIC };
	let colours = effects::frame(settings, shown, time);
	if playing && state.still.is_none() {
		ui.ctx()
			.request_repaint_after(std::time::Duration::from_millis(16));
	}
	let brightness = settings.light_brightness() as f32 / 255.0;
	for (led, texture) in &media.lights {
		let [r, g, b] = colours[*led];
		let level = |c: u8| (c as f32 * brightness).round() as u8;
		let tint = Color32::from_rgb(level(r), level(g), level(b));
		if tint != Color32::BLACK {
			ui.painter().image(texture.id(), picture, uv, tint);
		}
	}

	// Lines go below the buttons and dots, but their colour depends on which
	// button the pointer is over, so they are filled in afterwards.
	let lines = ui.painter().add(Shape::Noop);
	let mut line_shapes = Vec::new();
	let mut dots: Vec<(Pos2, bool)> = Vec::new();
	let buttons = if effects::has_own_colours(settings.light_effect()) {
		Vec::new()
	} else {
		callouts(single_leds)
	};
	for callout in buttons {
		let side = if callout.right_side { 1.0 } else { -1.0 };
		let edge = if callout.right_side {
			picture.right() + SWATCH_GAP
		} else {
			picture.left() - SWATCH_GAP - SWATCH_SIZE.x
		};
		let rect = Rect::from_min_size(
			pos2(
				edge,
				picture.top() + callout.y * scale - SWATCH_SIZE.y / 2.0,
			),
			SWATCH_SIZE,
		);
		let active = colour_button(ui, rect, settings, &callout.leds).hovered();
		let start = if callout.right_side {
			rect.left_center()
		} else {
			rect.right_center()
		};
		let elbow = start - vec2(10.0 * side, 0.0);
		let stroke = if active {
			Stroke::new(1.5, theme::ACCENT)
		} else {
			Stroke::new(1.0, theme::LINE)
		};
		for (x, y) in &callout.dots {
			let dot = picture.min + vec2(*x, *y) * scale;
			line_shapes.push((active, Shape::line(vec![start, elbow, dot], stroke)));
			dots.push((dot, active));
		}
	}
	line_shapes.sort_by_key(|(active, _)| *active);
	ui.painter().set(
		lines,
		Shape::Vec(line_shapes.into_iter().map(|(_, shape)| shape).collect()),
	);
	for (dot, active) in dots {
		let painter = ui.painter();
		if active {
			painter.circle_filled(dot, 4.5, theme::ACCENT);
			painter.circle_stroke(dot, 4.5, Stroke::new(1.0, Color32::WHITE));
		} else {
			painter.circle_filled(dot, 3.0, Color32::from_rgb(0xd0, 0xd4, 0xdb));
			painter.circle_stroke(dot, 3.0, Stroke::new(1.0, Color32::BLACK));
		}
	}

	let name = LIGHT_EFFECTS
		.iter()
		.find(|(number, _)| *number == effect)
		.map_or("This effect", |(_, name)| *name);
	let note = if effect == EFFECT_STATIC {
		"Preview of the stored colours.".to_owned()
	} else if playing {
		format!("{name}, simulated. The mouse may differ in detail.")
	} else {
		let why = if effects::is_animated(effect) {
			"is not playing"
		} else {
			"cannot be previewed"
		};
		format!("{name} {why}. Shown are the stored colours.")
	};
	ui.painter().text(
		pos2(area.center().x, area.bottom() - note_height / 2.0),
		egui::Align2::CENTER_CENTER,
		note,
		egui::FontId::proportional(12.5),
		theme::TEXT_DIM,
	);
}

fn controls(ui: &mut egui::Ui, settings: &mut Settings) {
	ui.add_space(4.0);
	ui.heading("Lighting");
	ui.add_space(8.0);

	theme::caption(ui, "Effect");
	let mut effect = settings.light_effect();
	let effect_name = LIGHT_EFFECTS
		.iter()
		.find(|(number, _)| *number == effect)
		.map_or_else(|| format!("Effect {effect}"), |(_, name)| name.to_string());
	egui::ComboBox::from_id_salt("effect")
		.width(ui.available_width())
		.selected_text(effect_name)
		.show_ui(ui, |ui| {
			// AIMO is left out: it is made for ROCCAT's software running on
			// the computer, which is out of scope.
			for (number, name) in LIGHT_EFFECTS {
				if *number != EFFECT_AIMO {
					ui.selectable_value(&mut effect, *number, *name);
				}
			}
		});
	if effect != settings.light_effect() {
		settings.set_light_effect(effect);
	}
	if effect == EFFECT_AIMO {
		ui.label(
			theme::dim(
				"AIMO is the factory setting. openRoc does not offer it: once you pick \
				 another effect, only a backup file brings it back.",
			)
			.size(12.0),
		);
	}

	ui.add_space(10.0);
	egui::Grid::new("light-levels")
		.num_columns(2)
		.spacing([14.0, 10.0])
		.show(ui, |ui| {
			ui.label("Brightness");
			let mut brightness = settings.light_brightness();
			if ui
				.add(egui::Slider::new(&mut brightness, 0..=255).show_value(false))
				.changed()
			{
				settings.set_light_brightness(brightness);
			}
			ui.end_row();

			let uses_speed = effects::uses_speed(effect);
			ui.add_enabled(uses_speed, egui::Label::new("Speed"));
			let mut speed = settings.light_speed();
			let range = LIGHT_SPEED_MIN..=LIGHT_SPEED_MAX;
			let changed = ui
				.horizontal(|ui| {
					// Narrower than the brightness slider: the number needs room.
					ui.spacing_mut().slider_width = 132.0;
					ui.add_enabled(uses_speed, egui::Slider::new(&mut speed, range))
						.on_disabled_hover_text("This effect looks the same at every speed.")
						.changed()
				})
				.inner;
			if changed {
				settings.set_light_speed(speed);
			}
			ui.end_row();
		});

	ui.add_space(16.0);
	theme::caption(ui, "Colours");
	if effects::has_own_colours(effect) {
		ui.label(theme::dim(
			"This effect brings its own colours. The colours stored in the profile \
			 are kept and show again with another effect.",
		));
		return;
	}
	ui.horizontal(|ui| {
		let (rect, _) = ui.allocate_exact_size(SWATCH_SIZE, Sense::hover());
		let all: Vec<usize> = (0..LED_COUNT).collect();
		colour_button(ui, rect, settings, &all);
		ui.label("All lights");
	});
	ui.add_space(6.0);
	ui.label(
		egui::RichText::new("The colour buttons around the mouse set single lights.")
			.size(11.5)
			.color(theme::TEXT_DIM),
	);
}
