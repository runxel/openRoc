//! Developer tool for protocol experiments on a connected mouse.
//!
//! ```text
//! cargo run -p openroc-core --example lab -- effect <profile 1-5> <number> [speed]
//! cargo run -p openroc-core --example lab -- leds <profile 1-5> <led>[,<led>…]:<rrggbb> …
//! cargo run -p openroc-core --example lab -- restore <settings.bin>
//! cargo run -p openroc-core --example lab -- activate <profile 1-5>
//! ```
//!
//! `effect` and `leds` change one profile and make it the active one so the
//! result is visible. `restore` writes a settings dump (such as the files in
//! `backup/`) back to the profile it was read from.

use std::error::Error;

use openroc_core::profile::{EFFECT_STATIC, LED_COUNT};
use openroc_core::{Device, Settings};

fn main() {
	let args: Vec<String> = std::env::args().skip(1).collect();
	let args: Vec<&str> = args.iter().map(String::as_str).collect();
	if let Err(e) = run(&args) {
		eprintln!("error: {e}");
		std::process::exit(1);
	}
}

fn profile_index(arg: &str, count: u8) -> Result<u8, Box<dyn Error>> {
	let number: u8 = arg.parse()?;
	if number == 0 || number > count {
		return Err(format!("profile must be 1 to {count}").into());
	}
	Ok(number - 1)
}

fn run(args: &[&str]) -> Result<(), Box<dyn Error>> {
	let device = Device::open()?;
	let count = device.profile_info()?.count;
	match args {
		["effect", profile, number, rest @ ..] => {
			let index = profile_index(profile, count)?;
			let mut settings = device.read_settings(index)?;
			let before = settings.light_effect();
			settings.set_light_effect(number.parse()?);
			if let [speed] = rest {
				settings.set_light_speed(speed.parse()?);
			}
			device.write_settings(&settings)?;
			device.set_active_profile(index)?;
			println!(
				"profile {profile}: effect {before} -> {}, speed {}, written, verified, active",
				settings.light_effect(),
				settings.light_speed()
			);
		}
		["leds", profile, specs @ ..] if !specs.is_empty() => {
			let index = profile_index(profile, count)?;
			let mut settings = device.read_settings(index)?;
			settings.set_light_effect(EFFECT_STATIC);
			for led in 0..LED_COUNT {
				settings.set_led_color(led, [0; 3]);
			}
			for spec in specs {
				let (leds, color) = spec
					.split_once(':')
					.ok_or("expected <led>[,<led>…]:<rrggbb>")?;
				let rgb = u32::from_str_radix(color, 16)?.to_be_bytes();
				for led in leds.split(',') {
					let led: usize = led.parse()?;
					if led == 0 || led > LED_COUNT {
						return Err(format!("LEDs are numbered 1 to {LED_COUNT}").into());
					}
					settings.set_led_color(led - 1, [rgb[1], rgb[2], rgb[3]]);
				}
			}
			device.write_settings(&settings)?;
			device.set_active_profile(index)?;
			println!("profile {profile}: {}, all others off", specs.join(" "));
		}
		["restore", path] => {
			let settings = Settings::from_bytes(&std::fs::read(path)?)?;
			device.write_settings(&settings)?;
			println!(
				"profile {} restored from {path}, verified",
				settings.profile_index() + 1
			);
		}
		["activate", profile] => {
			device.set_active_profile(profile_index(profile, count)?)?;
			println!("profile {profile} is now active");
		}
		_ => return Err("usage: lab effect|leds|restore|activate … (see the file header)".into()),
	}
	Ok(())
}
