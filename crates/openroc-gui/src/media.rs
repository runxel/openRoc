//! The pictures of the mouse, built into the app.
//!
//! They come from ROCCAT Swarm's Kone XP module: the top view without
//! branding, the enlarged view of the four thumb buttons, and one glow layer
//! per LED. `assets/` holds them under neutral names.

use eframe::egui::{self, Color32, ColorImage, TextureHandle, TextureOptions};
use openroc_core::profile::{LED_BUTTON, LED_FILL_LEFT, LED_FILL_RIGHT, LED_WHEEL, stripe_led};

/// Pixel size of the top view and of every light layer.
pub const MOUSE_SIZE: [f32; 2] = [340.0, 518.0];
/// Pixel size of the enlarged view of the thumb buttons.
pub const THUMB_BUTTONS_SIZE: [f32; 2] = [85.0, 188.0];

const MOUSE: &[u8] = include_bytes!("../assets/mouse.png");
const THUMB_BUTTONS: &[u8] = include_bytes!("../assets/thumb_buttons.png");

macro_rules! light {
	($file:literal, $led:expr) => {
		(
			$file,
			include_bytes!(concat!("../assets/", $file)) as &[u8],
			$led,
		)
	};
}

/// Swarm's light layers and the LED entry each one shows. The stripe layers
/// were matched to the LED positions tested on a mouse: odd numbers are palm
/// ends, even numbers cable ends. `i1` and `j1` are assumed to be the two
/// fill lights.
const LIGHTS: [(&str, &[u8], usize); 20] = [
	light!("light_01.png", stripe_led(false, 0, true)),
	light!("light_02.png", stripe_led(false, 0, false)),
	light!("light_03.png", stripe_led(false, 1, true)),
	light!("light_04.png", stripe_led(false, 1, false)),
	light!("light_05.png", stripe_led(false, 2, true)),
	light!("light_06.png", stripe_led(false, 2, false)),
	light!("light_07.png", stripe_led(false, 3, true)),
	light!("light_08.png", stripe_led(false, 3, false)),
	light!("light_09.png", stripe_led(true, 0, true)),
	light!("light_10.png", stripe_led(true, 0, false)),
	light!("light_11.png", stripe_led(true, 1, true)),
	light!("light_12.png", stripe_led(true, 1, false)),
	light!("light_13.png", stripe_led(true, 2, true)),
	light!("light_14.png", stripe_led(true, 2, false)),
	light!("light_15.png", stripe_led(true, 3, true)),
	light!("light_16.png", stripe_led(true, 3, false)),
	light!("light_wheel.png", LED_WHEEL),
	light!("light_button.png", LED_BUTTON),
	light!("light_i1.png", LED_FILL_LEFT),
	light!("light_j1.png", LED_FILL_RIGHT),
];

pub struct Media {
	/// Top view of the mouse without any light.
	pub mouse: TextureHandle,
	/// Enlarged view of the four thumb buttons.
	pub thumb_buttons: TextureHandle,
	/// One glow layer per LED, as (LED entry, texture). The textures are
	/// additive: painting one with a tint adds light of that colour.
	pub lights: Vec<(usize, TextureHandle)>,
}

impl Media {
	pub fn load(ctx: &egui::Context) -> Self {
		let picture = |name: &str, png: &[u8]| {
			let (size, rgba) = decode(png);
			let image = ColorImage::from_rgba_unmultiplied(size, &rgba);
			ctx.load_texture(name, image, TextureOptions::LINEAR)
		};
		let lights = LIGHTS
			.iter()
			.map(|(name, png, led)| {
				let (size, rgba) = decode(png);
				let pixels = rgba
					.as_chunks::<4>()
					.0
					.iter()
					.map(|p| {
						let brightest = p[0].max(p[1]).max(p[2]) as u16;
						let light = (brightest * p[3] as u16 / 255) as u8;
						Color32::from_rgba_premultiplied(light, light, light, 0)
					})
					.collect();
				let image = ColorImage::new(size, pixels);
				(*led, ctx.load_texture(*name, image, TextureOptions::LINEAR))
			})
			.collect();
		Self {
			mouse: picture("mouse", MOUSE),
			thumb_buttons: picture("thumb-buttons", THUMB_BUTTONS),
			lights,
		}
	}
}

/// Decodes one of the built-in PNG files to 8-bit RGBA.
fn decode(png: &[u8]) -> ([usize; 2], Vec<u8>) {
	const VALID: &str = "built-in picture is a valid PNG";
	let mut decoder = png::Decoder::new(std::io::Cursor::new(png));
	decoder.set_transformations(png::Transformations::normalize_to_color8());
	let mut reader = decoder.read_info().expect(VALID);
	let mut buffer = vec![0; reader.output_buffer_size().expect(VALID)];
	let info = reader.next_frame(&mut buffer).expect(VALID);
	buffer.truncate(info.buffer_size());
	let rgba = match info.color_type {
		png::ColorType::Rgba => buffer,
		png::ColorType::Rgb => buffer
			.as_chunks::<3>()
			.0
			.iter()
			.flat_map(|p| [p[0], p[1], p[2], 255])
			.collect(),
		other => panic!("built-in picture has unexpected colour type {other:?}"),
	};
	([info.width as usize, info.height as usize], rgba)
}

/// The app icon (window and Dock), built into the binary.
pub fn app_icon() -> eframe::egui::IconData {
	let ([width, height], rgba) = decode(include_bytes!("../assets/icon.png"));
	eframe::egui::IconData {
		rgba,
		width: width as u32,
		height: height as u32,
	}
}
