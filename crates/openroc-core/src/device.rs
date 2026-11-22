//! HID transport: feature reports on the vendor interface (usage page 0xFF01).

use std::thread::sleep;
use std::time::Duration;

use hidapi::{HidApi, HidDevice};

use crate::profile::{
	BUTTONS_LEN, Buttons, ProfileError, REPORT_BUTTONS, REPORT_SETTINGS, SETTINGS_LEN, Settings,
};

pub const VENDOR_ID: u16 = 0x1e7d;
pub const PRODUCT_ID: u16 = 0x2c8b;
const USAGE_PAGE_CONFIG: u16 = 0xff01;

const REPORT_CONTROL: u8 = 0x04;
const REPORT_PROFILE_INFO: u8 = 0x05;
const REPORT_DEVICE_INFO: u8 = 0x09;
const REQUEST_SETTINGS: u8 = 0x80;
const REQUEST_BUTTONS: u8 = 0x90;

const STATUS_OK: u8 = 1;
const STATUS_FAILED: u8 = 2;
const STATUS_BUSY: u8 = 3;
const POLL_INTERVAL: Duration = Duration::from_millis(25);
/// Swarm gives up after 100 polls of 150 ms; keep the same overall budget.
const POLL_LIMIT: u32 = 600;

#[derive(Debug, thiserror::Error)]
pub enum DeviceError {
	#[error("no ROCCAT Kone XP found")]
	NotFound,
	#[error("USB communication failed: {0}")]
	Hid(#[from] hidapi::HidError),
	#[error("the mouse rejected the data")]
	Rejected,
	#[error("the mouse stayed busy")]
	Timeout,
	#[error("unexpected status {0:#04x} from the mouse")]
	Status(u8),
	#[error("the mouse returned invalid data: {0}")]
	Invalid(#[from] ProfileError),
	#[error("the mouse answered for profile {got} instead of {expected}")]
	WrongProfile { expected: u8, got: u8 },
	#[error("read-back after writing differs from what was sent")]
	VerifyFailed,
	#[error("profile {0} does not exist")]
	NoSuchProfile(u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProfileInfo {
	pub active: u8,
	pub count: u8,
}

pub struct Device {
	hid: HidDevice,
}

impl Device {
	/// Opens the configuration interface of the first Kone XP found.
	pub fn open() -> Result<Self, DeviceError> {
		let api = HidApi::new()?;
		let info = api
			.device_list()
			.find(|d| {
				d.vendor_id() == VENDOR_ID
					&& d.product_id() == PRODUCT_ID
					&& d.usage_page() == USAGE_PAGE_CONFIG
			})
			.ok_or(DeviceError::NotFound)?;
		Ok(Self {
			hid: info.open_device(&api)?,
		})
	}

	fn get(&self, report_id: u8, len: usize) -> Result<Vec<u8>, DeviceError> {
		let mut buf = vec![0u8; len];
		buf[0] = report_id;
		let n = self.hid.get_feature_report(&mut buf)?;
		buf.truncate(n);
		Ok(buf)
	}

	/// Sends a feature report and waits until the mouse reports it as processed.
	fn send(&self, report: &[u8]) -> Result<(), DeviceError> {
		self.hid.send_feature_report(report)?;
		for _ in 0..POLL_LIMIT {
			sleep(POLL_INTERVAL);
			let status = self.get(REPORT_CONTROL, 4)?;
			match status.get(1).copied() {
				Some(STATUS_OK) => return Ok(()),
				Some(STATUS_FAILED) => return Err(DeviceError::Rejected),
				Some(STATUS_BUSY) => continue,
				other => return Err(DeviceError::Status(other.unwrap_or(0))),
			}
		}
		Err(DeviceError::Timeout)
	}

	pub fn firmware_version(&self) -> Result<String, DeviceError> {
		let info = self.get(REPORT_DEVICE_INFO, 9)?;
		let v = info.get(2).copied().unwrap_or(0);
		Ok(format!("{}.{:02}", v / 100, v % 100))
	}

	pub fn profile_info(&self) -> Result<ProfileInfo, DeviceError> {
		let r = self.get(REPORT_PROFILE_INFO, 4)?;
		if r.len() < 4 {
			return Err(DeviceError::Status(0));
		}
		Ok(ProfileInfo {
			active: r[2],
			count: r[3],
		})
	}

	pub fn set_active_profile(&self, index: u8) -> Result<(), DeviceError> {
		let info = self.profile_info()?;
		if index >= info.count {
			return Err(DeviceError::NoSuchProfile(index));
		}
		self.send(&[REPORT_PROFILE_INFO, 4, index, info.count])
	}

	fn read_profile_report(
		&self,
		index: u8,
		request: u8,
		report_id: u8,
		len: usize,
	) -> Result<Vec<u8>, DeviceError> {
		self.send(&[REPORT_CONTROL, index, request, 0])?;
		let raw = self.get(report_id, len)?;
		match raw.get(2).copied() {
			Some(got) if got != index => Err(DeviceError::WrongProfile {
				expected: index,
				got,
			}),
			_ => Ok(raw),
		}
	}

	pub fn read_settings(&self, index: u8) -> Result<Settings, DeviceError> {
		let raw =
			self.read_profile_report(index, REQUEST_SETTINGS, REPORT_SETTINGS, SETTINGS_LEN)?;
		Ok(Settings::from_bytes(&raw)?)
	}

	pub fn read_buttons(&self, index: u8) -> Result<Buttons, DeviceError> {
		let raw = self.read_profile_report(index, REQUEST_BUTTONS, REPORT_BUTTONS, BUTTONS_LEN)?;
		Ok(Buttons::from_bytes(&raw)?)
	}

	/// Writes the settings to the profile they belong to and verifies the result.
	pub fn write_settings(&self, settings: &Settings) -> Result<(), DeviceError> {
		self.send(&settings.to_bytes())?;
		if self.read_settings(settings.profile_index())?.to_bytes() != settings.to_bytes() {
			return Err(DeviceError::VerifyFailed);
		}
		Ok(())
	}

	/// Writes the buttons to the profile they belong to and verifies the result.
	pub fn write_buttons(&self, buttons: &Buttons) -> Result<(), DeviceError> {
		self.send(&buttons.to_bytes())?;
		if self.read_buttons(buttons.profile_index())?.to_bytes() != buttons.to_bytes() {
			return Err(DeviceError::VerifyFailed);
		}
		Ok(())
	}
}
