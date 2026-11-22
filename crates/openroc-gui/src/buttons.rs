//! The Buttons tab: the mouse with a callout per input, and the inspector
//! that changes the selected binding.

use eframe::egui::{
	self, Align, Color32, CornerRadius, Pos2, Rect, RichText, Sense, Shape, Stroke, StrokeKind,
	UiBuilder, epaint::RectShape, pos2, vec2,
};
use openroc_core::keys::{KEYS, Layout as KeyLayout, MODIFIERS};
use openroc_core::profile::{INPUT_COUNT, INPUT_NAMES, KNOWN_FUNCTIONS, KnownFunction};
use openroc_core::{Binding, Buttons};

use crate::media::{MOUSE_SIZE, Media, THUMB_BUTTONS_SIZE};
use crate::theme;

/// What the Buttons tab remembers between frames.
#[derive(Default)]
pub struct State {
	/// 0 = normal layer, 1 = Easy-Shift layer.
	pub layer: usize,
	/// The input whose binding is shown in the inspector.
	pub input: Option<usize>,
}

/// The button view is drawn on a canvas of this size: the top view of the
/// mouse with the enlarged thumb buttons to its left.
const CANVAS_SIZE: egui::Vec2 = vec2(440.0, 518.0);
const MOUSE_AT: egui::Vec2 = vec2(100.0, 0.0);
const THUMB_BUTTONS_AT: egui::Vec2 = vec2(0.0, 190.0);

/// Where each input sits on the canvas, in canvas pixels.
const HOTSPOTS: [(f32, f32); INPUT_COUNT] = [
	(232.0, 100.0), // left button
	(322.0, 100.0), // right button
	(275.0, 141.0), // wheel click
	(262.0, 149.0), // wheel tilt left
	(288.0, 132.0), // wheel tilt right
	(275.0, 118.0), // wheel up
	(275.0, 164.0), // wheel down
	(196.0, 116.0), // "+" button
	(184.0, 160.0), // "−" button
	// The thumb buttons sit side by side in pairs. The markers are staggered
	// so that no line runs through the marker of the neighbouring button.
	(60.0, 225.0),  // thumb T1
	(60.0, 300.0),  // thumb T2
	(37.0, 255.0),  // thumb T3
	(37.0, 330.0),  // thumb T4
	(142.0, 285.0), // Easy-Shift button, the flat paddle below the thumb buttons
	(275.0, 272.0), // behind the wheel
];

/// Callouts left and right of the picture, each from top to bottom. The order
/// follows the hotspots so that the lines do not cross.
const LEFT_CALLOUTS: [usize; 8] = [0, 7, 8, 9, 11, 10, 12, 13];
const RIGHT_CALLOUTS: [usize; 7] = [1, 5, 4, 2, 3, 6, 14];

const CALLOUT_SIZE: egui::Vec2 = vec2(164.0, 46.0);
const CALLOUT_GAP: f32 = 8.0;

pub fn show(
	ui: &mut egui::Ui,
	media: &Media,
	buttons: &mut Buttons,
	state: &mut State,
	layout: &KeyLayout,
) {
	egui::Panel::right("inspector")
		.resizable(false)
		.exact_size(290.0)
		.frame(theme::side_panel())
		.show(ui, |ui| inspector(ui, buttons, state, layout));

	egui::CentralPanel::default()
		.frame(egui::Frame::new().inner_margin(egui::Margin::same(16)))
		.show(ui, |ui| {
			ui.horizontal(|ui| {
				theme::segmented(
					ui,
					&mut state.layer,
					&[(0, "Normal"), (1, "With Easy-Shift held")],
				);
				if !buttons.has_left_click() {
					ui.add_space(8.0);
					ui.colored_label(
						theme::WARNING,
						"No button is a left click. You would need another pointing device to undo this.",
					);
				}
			});
			ui.add_space(8.0);
			callouts(ui, media, buttons, state, layout);
		});
}

/// The mouse picture with one clickable card per input.
fn callouts(
	ui: &mut egui::Ui,
	media: &Media,
	buttons: &Buttons,
	state: &mut State,
	layout: &KeyLayout,
) {
	let area = ui.available_rect_before_wrap();
	ui.allocate_rect(area, Sense::hover());

	let picture_space = vec2(area.width() - 2.0 * (CALLOUT_SIZE.x + 14.0), area.height());
	let scale = (picture_space.x / CANVAS_SIZE.x)
		.min(picture_space.y / CANVAS_SIZE.y)
		.clamp(0.4, 1.3);
	let picture = Rect::from_center_size(area.center(), CANVAS_SIZE * scale);
	let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
	for (texture, at, size) in [
		(&media.mouse, MOUSE_AT, MOUSE_SIZE),
		(&media.thumb_buttons, THUMB_BUTTONS_AT, THUMB_BUTTONS_SIZE),
	] {
		let rect = Rect::from_min_size(picture.min + at * scale, vec2(size[0], size[1]) * scale);
		ui.painter().image(texture.id(), rect, uv, Color32::WHITE);
	}

	// Lines first, so that the dots and cards cover their ends.
	let lines = ui.painter().add(Shape::Noop);
	let mut line_shapes = Vec::new();
	let mut dots = Vec::new();

	for (side, inputs) in [(-1.0, &LEFT_CALLOUTS[..]), (1.0, &RIGHT_CALLOUTS[..])] {
		let count = inputs.len() as f32;
		let height = count * CALLOUT_SIZE.y + (count - 1.0) * CALLOUT_GAP;
		let top = area.center().y - height / 2.0;
		let left = if side < 0.0 {
			area.left()
		} else {
			area.right() - CALLOUT_SIZE.x
		};
		for (row, &input) in inputs.iter().enumerate() {
			let rect = Rect::from_min_size(
				pos2(left, top + row as f32 * (CALLOUT_SIZE.y + CALLOUT_GAP)),
				CALLOUT_SIZE,
			);
			let slot = input + state.layer * INPUT_COUNT;
			let locked = is_locked(buttons, state.layer, input);
			let selected = state.input == Some(input) && !locked;
			let action = if locked {
				"Held down".to_string()
			} else {
				buttons.slot(slot).label(layout)
			};
			let response = callout(ui, rect, INPUT_NAMES[input], &action, selected, !locked);
			if response.clicked() && !locked {
				state.input = Some(input);
			}
			let active = selected || (response.hovered() && !locked);

			let (x, y) = HOTSPOTS[input];
			let dot = picture.min + vec2(x, y) * scale;
			let start = if side < 0.0 {
				rect.right_center()
			} else {
				rect.left_center()
			};
			let elbow = start + vec2(14.0 * side, 0.0);
			let stroke = if active {
				Stroke::new(1.5, theme::ACCENT)
			} else {
				Stroke::new(1.0, theme::LINE)
			};
			line_shapes.push((active, Shape::line(vec![start, elbow, dot], stroke)));
			dots.push((dot, active));
		}
	}

	// Highlighted lines go on top of the others.
	line_shapes.sort_by_key(|(active, _)| *active);
	ui.painter().set(
		lines,
		Shape::Vec(line_shapes.into_iter().map(|(_, shape)| shape).collect()),
	);
	for (dot, active) in dots {
		paint_dot(ui, dot, active);
	}
}

fn paint_dot(ui: &egui::Ui, center: Pos2, active: bool) {
	let painter = ui.painter();
	if active {
		painter.circle_filled(center, 6.0, theme::ACCENT);
		painter.circle_stroke(center, 6.0, Stroke::new(1.5, Color32::WHITE));
	} else {
		painter.circle_filled(center, 4.0, Color32::from_rgb(0xd0, 0xd4, 0xdb));
		painter.circle_stroke(center, 4.0, Stroke::new(1.0, Color32::BLACK));
	}
}

/// One card: the input's name and what it currently does.
fn callout(
	ui: &mut egui::Ui,
	rect: Rect,
	name: &str,
	action: &str,
	selected: bool,
	enabled: bool,
) -> egui::Response {
	let background = ui.painter().add(Shape::Noop);
	ui.scope_builder(
		UiBuilder::new()
			.max_rect(rect.shrink2(vec2(12.0, 6.0)))
			.layout(egui::Layout::top_down(Align::Min)),
		|ui| {
			ui.spacing_mut().item_spacing.y = 1.0;
			ui.add(
				egui::Label::new(RichText::new(name).size(11.5).color(theme::TEXT_DIM))
					.truncate()
					.selectable(false),
			);
			let color = if selected {
				theme::ACCENT
			} else if enabled {
				theme::TEXT
			} else {
				theme::TEXT_DIM
			};
			ui.add(
				egui::Label::new(RichText::new(action).size(14.0).color(color))
					.truncate()
					.selectable(false),
			);
		},
	);
	// Added last so that it lies on top of the labels and gets the click.
	let sense = if enabled {
		Sense::click()
	} else {
		Sense::hover()
	};
	let response = ui.interact(rect, egui::Id::unique(("callout", name)), sense);
	let (fill, stroke) = if selected {
		(theme::ACCENT_FILL, Stroke::new(1.5, theme::ACCENT))
	} else if !enabled {
		(theme::PANEL, Stroke::new(1.0, theme::BORDER))
	} else if response.hovered() {
		(theme::CARD_HOVER, Stroke::new(1.0, theme::LINE))
	} else {
		(theme::CARD, Stroke::new(1.0, theme::BORDER))
	};
	ui.painter().set(
		background,
		RectShape::new(
			rect,
			CornerRadius::same(theme::RADIUS),
			fill,
			stroke,
			StrokeKind::Inside,
		),
	);
	if enabled {
		response.on_hover_cursor(egui::CursorIcon::PointingHand)
	} else {
		response.on_hover_text(
			"This button is Easy-Shift. It is held down to reach these functions, so it cannot \
			 have one of its own here.",
		)
	}
}

/// True for the button that is Easy-Shift, while looking at the functions that
/// apply with Easy-Shift held: it is already pressed and cannot do more.
fn is_locked(buttons: &Buttons, layer: usize, input: usize) -> bool {
	layer == 1 && buttons.slot(input) == Binding::EASY_SHIFT
}

/// The groups the action list is divided into.
const CATEGORIES: [&str; 6] = [
	"Mouse",
	"Keyboard",
	"DPI",
	"Profile and lighting",
	"Media",
	"Browser",
];

fn category(function: &KnownFunction) -> Option<usize> {
	let [_, _, number, group] = function.binding.0;
	match (group, number) {
		(0x00, _) => None,
		(0x01, 0x05 | 0x06) => Some(5),
		(0x01, _) => Some(0),
		(0x04, _) => Some(1),
		(0x02, _) => Some(2),
		(0x08 | 0x0a, _) => Some(3),
		(0x03, 0x0b..) => Some(5),
		(0x03, _) => Some(4),
		_ => None,
	}
}

fn inspector(ui: &mut egui::Ui, buttons: &mut Buttons, state: &mut State, layout: &KeyLayout) {
	let Some(input) = state.input else {
		ui.add_space(4.0);
		ui.heading("Buttons");
		ui.add_space(4.0);
		ui.label(theme::dim(
			"Select a button to change what it does. Each button has a second function that \
			 applies while the Easy-Shift button is held.",
		));
		return;
	};
	let slot = input + state.layer * INPUT_COUNT;
	let current = buttons.slot(slot);
	let mut binding = current;

	ui.add_space(4.0);
	ui.heading(INPUT_NAMES[input]);
	if is_locked(buttons, state.layer, input) {
		ui.label(theme::dim(
			"This button is Easy-Shift. It is held down to reach the second functions of the \
			 other buttons, so it cannot have one itself.",
		));
		return;
	}
	ui.label(theme::dim(if state.layer == 0 {
		"Normal function"
	} else {
		"Function while Easy-Shift is held"
	}));
	ui.add_space(8.0);

	egui::ScrollArea::vertical()
		.auto_shrink([false, false])
		.show(ui, |ui| {
			ui.spacing_mut().item_spacing.y = 2.0;
			if theme::choice_row(ui, binding == Binding::function(0, 0), "Disabled", "").clicked() {
				binding = Binding::function(0, 0);
			}
			for (index, title) in CATEGORIES.iter().enumerate() {
				ui.add_space(10.0);
				theme::caption(ui, title);
				if index == 1 {
					if theme::choice_row(ui, binding.is_key(), "Keyboard shortcut", "").clicked()
						&& !binding.is_key()
					{
						binding = Binding::key(KEYS[0].0, 0);
					}
					if binding.is_key() {
						shortcut_editor(ui, &mut binding, layout);
					}
				}
				for function in KNOWN_FUNCTIONS
					.iter()
					.filter(|f| category(f) == Some(index))
				{
					let note = if function.verified { "" } else { "untested" };
					let selected = binding == function.binding;
					if theme::choice_row(ui, selected, function.name, note).clicked() {
						binding = function.binding;
					}
				}
			}
			if !binding.is_key() && binding.known().is_none() {
				ui.add_space(10.0);
				theme::caption(ui, "Other");
				theme::choice_row(ui, true, &binding.label(layout), "");
			}
			ui.add_space(12.0);
			let b = binding.0;
			ui.label(
				RichText::new(format!(
					"Stored as {:02x} {:02x} {:02x} {:02x}",
					b[0], b[1], b[2], b[3]
				))
				.size(11.5)
				.color(theme::TEXT_DIM),
			);
		});

	if binding != current {
		buttons.set_slot(slot, binding);
	}
}

/// Modifier toggles and the key of a keyboard shortcut.
fn shortcut_editor(ui: &mut egui::Ui, binding: &mut Binding, layout: &KeyLayout) {
	let mut usage = binding.key_usage();
	let mut modifiers = binding.key_modifiers();
	egui::Frame::new()
		.fill(theme::CARD)
		.corner_radius(CornerRadius::same(7))
		.inner_margin(egui::Margin::same(10))
		.show(ui, |ui| {
			ui.spacing_mut().item_spacing = vec2(6.0, 8.0);
			ui.horizontal(|ui| {
				for (bit, name) in MODIFIERS {
					let on = modifiers & bit != 0;
					if ui.add(egui::Button::selectable(on, name)).clicked() {
						modifiers ^= bit;
					}
				}
			});
			ui.horizontal(|ui| {
				ui.label(theme::dim("Key"));
				egui::ComboBox::from_id_salt("key")
					.width(150.0)
					.selected_text(layout.key_name(usage))
					.show_ui(ui, |ui| {
						for (code, name) in layout.keys() {
							ui.selectable_value(&mut usage, code, name);
						}
					});
			});
			let layout_name = if layout.name.is_empty() {
				"US"
			} else {
				layout.name.as_str()
			};
			ui.label(
				RichText::new(format!(
					"Keys are named for your keyboard layout ({layout_name}). The mouse stores \
					 the key's position, so it types something else under another layout."
				))
				.size(11.5)
				.color(theme::TEXT_DIM),
			);
		});
	*binding = Binding::key(usage, modifiers);
}
