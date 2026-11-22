//! Colours, fonts and the few shared widgets that give the app its look.

use std::sync::Arc;

use eframe::egui::{
	self, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Frame, Margin,
	RichText, Sense, Stroke, StrokeKind, TextStyle, vec2,
};

pub const BACKGROUND: Color32 = Color32::from_rgb(0x12, 0x13, 0x16);
pub const PANEL: Color32 = Color32::from_rgb(0x18, 0x1a, 0x1e);
pub const CARD: Color32 = Color32::from_rgb(0x21, 0x24, 0x2a);
pub const CARD_HOVER: Color32 = Color32::from_rgb(0x2a, 0x2e, 0x36);
pub const BORDER: Color32 = Color32::from_rgb(0x33, 0x37, 0x3f);
pub const LINE: Color32 = Color32::from_rgb(0x5a, 0x60, 0x6b);
pub const TEXT: Color32 = Color32::from_rgb(0xe6, 0xe8, 0xec);
pub const TEXT_DIM: Color32 = Color32::from_rgb(0x91, 0x98, 0xa3);
pub const ACCENT: Color32 = Color32::from_rgb(0x3b, 0xa4, 0xff);
pub const ACCENT_FILL: Color32 = Color32::from_rgb(0x1c, 0x33, 0x4d);
pub const ON_ACCENT: Color32 = Color32::from_rgb(0x06, 0x16, 0x26);
pub const WARNING: Color32 = Color32::from_rgb(0xff, 0xb4, 0x54);

pub const RADIUS: u8 = 9;

/// The macOS system font, so the app reads like other Mac apps. egui's own
/// font stays as the fallback.
const SYSTEM_FONT: &str = "/System/Library/Fonts/SFNS.ttf";

pub fn apply(ctx: &egui::Context) {
	if let Ok(bytes) = std::fs::read(SYSTEM_FONT) {
		let mut fonts = FontDefinitions::default();
		fonts
			.font_data
			.insert("system".to_owned(), Arc::new(FontData::from_owned(bytes)));
		if let Some(family) = fonts.families.get_mut(&FontFamily::Proportional) {
			family.insert(0, "system".to_owned());
		}
		ctx.set_fonts(fonts);
	}

	ctx.set_theme(egui::Theme::Dark);
	let mut visuals = egui::Visuals::dark();
	visuals.panel_fill = BACKGROUND;
	visuals.window_fill = CARD;
	visuals.window_stroke = Stroke::new(1.0, BORDER);
	visuals.extreme_bg_color = Color32::from_rgb(0x0c, 0x0d, 0x0f);
	visuals.faint_bg_color = PANEL;
	visuals.override_text_color = Some(TEXT);
	visuals.warn_fg_color = WARNING;
	visuals.selection.bg_fill = ACCENT_FILL;
	visuals.selection.stroke = Stroke::new(1.0, ACCENT);
	visuals.hyperlink_color = ACCENT;
	let radius = CornerRadius::same(7);
	let widgets = &mut visuals.widgets;
	widgets.noninteractive.bg_stroke = Stroke::new(1.0, BORDER);
	widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT_DIM);
	widgets.noninteractive.corner_radius = radius;
	widgets.inactive.bg_fill = CARD_HOVER;
	widgets.inactive.weak_bg_fill = CARD;
	widgets.inactive.bg_stroke = Stroke::new(1.0, BORDER);
	widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT);
	widgets.inactive.corner_radius = radius;
	widgets.hovered.bg_fill = Color32::from_rgb(0x3a, 0x3f, 0x4a);
	widgets.hovered.weak_bg_fill = CARD_HOVER;
	widgets.hovered.bg_stroke = Stroke::new(1.0, LINE);
	widgets.hovered.fg_stroke = Stroke::new(1.0, Color32::WHITE);
	widgets.hovered.corner_radius = radius;
	widgets.active.bg_fill = ACCENT;
	widgets.active.weak_bg_fill = ACCENT_FILL;
	widgets.active.bg_stroke = Stroke::new(1.0, ACCENT);
	widgets.active.fg_stroke = Stroke::new(1.0, Color32::WHITE);
	widgets.active.corner_radius = radius;
	widgets.open.weak_bg_fill = CARD_HOVER;
	widgets.open.bg_stroke = Stroke::new(1.0, LINE);
	widgets.open.corner_radius = radius;
	ctx.set_visuals_of(egui::Theme::Dark, visuals);

	ctx.all_styles_mut(|style| {
		style.spacing.item_spacing = vec2(8.0, 8.0);
		style.spacing.button_padding = vec2(12.0, 6.0);
		style.spacing.interact_size.y = 28.0;
		style.spacing.slider_width = 180.0;
		style.spacing.combo_height = 320.0;
		style
			.text_styles
			.insert(TextStyle::Heading, FontId::proportional(22.0));
		style
			.text_styles
			.insert(TextStyle::Body, FontId::proportional(14.0));
		style
			.text_styles
			.insert(TextStyle::Button, FontId::proportional(14.0));
		style
			.text_styles
			.insert(TextStyle::Small, FontId::proportional(12.0));
	});
}

/// A rounded box that groups related controls.
pub fn card() -> Frame {
	Frame::new()
		.fill(CARD)
		.stroke(Stroke::new(1.0, BORDER))
		.corner_radius(CornerRadius::same(RADIUS))
		.inner_margin(Margin::same(16))
}

/// The frame of a side panel.
pub fn side_panel() -> Frame {
	Frame::new().fill(PANEL).inner_margin(Margin::same(16))
}

/// A small caption above a group of controls.
pub fn caption(ui: &mut egui::Ui, text: &str) {
	ui.label(RichText::new(text).size(12.0).color(TEXT_DIM));
}

pub fn dim(text: impl Into<String>) -> RichText {
	RichText::new(text).color(TEXT_DIM)
}

/// One row of mutually exclusive choices, like a macOS segmented control.
pub fn segmented<T: PartialEq + Copy>(ui: &mut egui::Ui, current: &mut T, options: &[(T, &str)]) {
	Frame::new()
		.fill(CARD)
		.stroke(Stroke::new(1.0, BORDER))
		.corner_radius(CornerRadius::same(RADIUS))
		.inner_margin(Margin::same(3))
		.show(ui, |ui| {
			ui.horizontal(|ui| {
				ui.spacing_mut().item_spacing.x = 2.0;
				for (value, label) in options {
					let selected = *current == *value;
					let text = RichText::new(*label).color(if selected { TEXT } else { TEXT_DIM });
					let button = egui::Button::new(text)
						.fill(if selected {
							Color32::from_rgb(0x3a, 0x3f, 0x4a)
						} else {
							Color32::TRANSPARENT
						})
						.stroke(Stroke::NONE)
						.corner_radius(CornerRadius::same(RADIUS - 3))
						.min_size(vec2(0.0, 26.0));
					if ui.add(button).clicked() {
						*current = *value;
					}
				}
			});
		});
}

/// The one button that carries out the main action of the window.
pub fn primary_button(ui: &mut egui::Ui, enabled: bool, label: &str) -> egui::Response {
	let text = RichText::new(label).color(if enabled { ON_ACCENT } else { TEXT_DIM });
	let button = egui::Button::new(text)
		.fill(if enabled { ACCENT } else { CARD })
		.stroke(Stroke::NONE);
	ui.add_enabled(enabled, button)
}

/// A full-width row that can be picked, with an optional note on the right.
pub fn choice_row(ui: &mut egui::Ui, selected: bool, label: &str, note: &str) -> egui::Response {
	let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 30.0), Sense::click());
	let fill = if selected {
		ACCENT_FILL
	} else if response.hovered() {
		CARD_HOVER
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
	painter.text(
		rect.left_center() + vec2(10.0, 0.0),
		egui::Align2::LEFT_CENTER,
		label,
		FontId::proportional(14.0),
		TEXT,
	);
	let note = if selected && note.is_empty() {
		"✓"
	} else {
		note
	};
	if !note.is_empty() {
		painter.text(
			rect.right_center() - vec2(10.0, 0.0),
			egui::Align2::RIGHT_CENTER,
			note,
			FontId::proportional(12.0),
			if selected { ACCENT } else { TEXT_DIM },
		);
	}
	response
}

/// A colour button at a fixed place; clicking it opens the colour picker.
/// `mixed` shows it as a grey box with a diagonal line instead, for a button
/// that stands for several lights which do not all have the same colour.
pub fn swatch(
	ui: &mut egui::Ui,
	rect: egui::Rect,
	rgb: &mut [u8; 3],
	mixed: bool,
) -> egui::Response {
	let response = ui.put(rect, |ui: &mut egui::Ui| {
		ui.spacing_mut().interact_size = rect.size();
		ui.color_edit_button_srgb(rgb)
	});
	if mixed {
		let painter = ui.painter();
		painter.rect(
			rect,
			CornerRadius::same(4),
			Color32::from_rgb(0x4a, 0x4f, 0x59),
			Stroke::new(1.0, LINE),
			StrokeKind::Inside,
		);
		painter.line_segment(
			[
				rect.left_bottom() + vec2(3.0, -3.0),
				rect.right_top() + vec2(-3.0, 3.0),
			],
			Stroke::new(1.5, TEXT),
		);
	}
	response
}
