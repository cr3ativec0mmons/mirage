//! Communication with the ADB server and wireless discovery of devices.

mod client;
mod devices;
mod tracker;

pub use client::{AdbClient, Error};
pub use devices::DeviceInfo;
pub use tracker::{TrackerEvent, TrackerHandle, spawn_tracker};
