//! Launching scrcpy, which opens its own window for each device.

mod options;
mod session;

pub use options::{Options, Recording};
pub use session::{Outcome, Session};
