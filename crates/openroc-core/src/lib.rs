//! Protocol and data model for the ROCCAT Kone XP configuration interface.
//!
//! See `docs/PROTOCOL.md` for what is known about the wire format and how sure
//! each part is.

pub mod device;
pub mod export;
pub mod keys;
pub mod profile;

pub use device::{Device, DeviceError, ProfileInfo};
pub use profile::{Binding, Buttons, ProfileError, Settings};
