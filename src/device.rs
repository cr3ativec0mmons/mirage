//! A device known to the ADB server.

use std::cell::{Cell, RefCell};

use gtk::glib;
use gtk::glib::subclass::prelude::*;
use gtk::prelude::*;

use crate::adb::DeviceInfo;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, glib::Enum)]
#[enum_type(name = "MirageDeviceState")]
pub enum DeviceState {
    /// Ready to be mirrored.
    #[default]
    Online,
    /// The user has to allow debugging on the device.
    Unauthorized,
    /// The computer has no permission to open the USB device.
    NoPermissions,
    Connecting,
    Offline,
    /// Bootloader, recovery, sideload and other modes that cannot be mirrored.
    Unavailable,
}

impl DeviceState {
    pub fn from_adb(state: &str) -> Self {
        match state {
            "device" => Self::Online,
            "unauthorized" => Self::Unauthorized,
            "authorizing" | "connecting" => Self::Connecting,
            "offline" => Self::Offline,
            state if state.starts_with("no permissions") => Self::NoPermissions,
            _ => Self::Unavailable,
        }
    }
}

/// What Mirage is doing with the device.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, glib::Enum)]
#[enum_type(name = "MirageActivity")]
pub enum Activity {
    #[default]
    Idle,
    Mirroring,
    Recording,
}

mod imp {
    use super::*;

    #[derive(Debug, Default, glib::Properties)]
    #[properties(wrapper_type = super::Device)]
    pub struct Device {
        #[property(get, construct_only)]
        serial: RefCell<String>,
        /// The name set on the device, or else its model.
        #[property(get)]
        name: RefCell<String>,
        #[property(get, builder(DeviceState::default()))]
        state: Cell<DeviceState>,
        /// The state as reported by ADB.
        #[property(get)]
        raw_state: RefCell<String>,
        #[property(get)]
        wireless: Cell<bool>,
        #[property(get, set, builder(Activity::default()))]
        activity: Cell<Activity>,

        pub model: RefCell<Option<String>>,
        pub custom_name: RefCell<Option<String>>,
        pub name_requested: Cell<bool>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Device {
        const NAME: &'static str = "MirageDevice";
        type Type = super::Device;
    }

    #[glib::derived_properties]
    impl ObjectImpl for Device {}

    impl Device {
        pub fn set_state(&self, state: DeviceState, raw_state: &str) {
            let obj = self.obj();
            if self.state.replace(state) != state {
                obj.notify_state();
            }
            if *self.raw_state.borrow() != raw_state {
                self.raw_state.replace(raw_state.to_owned());
                obj.notify_raw_state();
            }
        }

        pub fn set_wireless(&self, wireless: bool) {
            if self.wireless.replace(wireless) != wireless {
                self.obj().notify_wireless();
            }
        }

        pub fn update_name(&self) {
            let name = self
                .custom_name
                .borrow()
                .clone()
                .or_else(|| self.model.borrow().clone())
                .unwrap_or_else(|| self.serial.borrow().clone());
            if *self.name.borrow() != name {
                self.name.replace(name);
                self.obj().notify_name();
            }
        }
    }
}

glib::wrapper! {
    pub struct Device(ObjectSubclass<imp::Device>);
}

impl Device {
    pub fn new(info: &DeviceInfo) -> Self {
        let device: Self = glib::Object::builder()
            .property("serial", &info.serial)
            .build();
        device.update(info);
        device
    }

    pub fn update(&self, info: &DeviceInfo) {
        let imp = self.imp();
        imp.set_state(DeviceState::from_adb(&info.state), &info.state);
        imp.set_wireless(info.is_wireless());
        // The model is missing while the device is unauthorized
        if let Some(model) = info.model() {
            imp.model.replace(Some(model));
        }
        imp.update_name();
    }

    /// Sets the name chosen by the user in the settings of the device.
    pub fn set_custom_name(&self, name: Option<String>) {
        self.imp().custom_name.replace(name);
        self.imp().update_name();
    }

    /// Returns `true` the first time it is called, to look up the name once.
    pub fn take_name_request(&self) -> bool {
        !self.imp().name_requested.replace(true)
    }

    pub fn is_online(&self) -> bool {
        self.state() == DeviceState::Online
    }

    /// The host part of the serial of devices connected with `adb connect`.
    pub fn network_host(&self) -> Option<String> {
        if !self.wireless() {
            return None;
        }
        let serial = self.serial();
        let (host, port) = serial.rsplit_once(':')?;
        port.parse::<u16>().ok()?;
        Some(
            host.trim_start_matches('[')
                .trim_end_matches(']')
                .to_owned(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_adb_states() {
        assert_eq!(DeviceState::from_adb("device"), DeviceState::Online);
        assert_eq!(
            DeviceState::from_adb("unauthorized"),
            DeviceState::Unauthorized
        );
        assert_eq!(DeviceState::from_adb("offline"), DeviceState::Offline);
        assert_eq!(
            DeviceState::from_adb("authorizing"),
            DeviceState::Connecting
        );
        assert_eq!(
            DeviceState::from_adb("no permissions (missing udev rules?)"),
            DeviceState::NoPermissions
        );
        assert_eq!(DeviceState::from_adb("recovery"), DeviceState::Unavailable);
    }
}
