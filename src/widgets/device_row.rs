use std::cell::OnceCell;

use adw::prelude::*;
use adw::subclass::prelude::*;
use gettextrs::gettext;
use gtk::glib;
use gtk::glib::clone;

use crate::device::{Activity, Device, DeviceState};
use crate::i18n::gettext_f;

mod imp {
    use super::*;

    #[derive(Debug, Default, gtk::CompositeTemplate)]
    #[template(resource = "/tech/vitainside/mirage/ui/device-row.ui")]
    pub struct MirageDeviceRow {
        #[template_child]
        pub connection_icon: TemplateChild<gtk::Image>,
        #[template_child]
        pub record_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub mirror_button: TemplateChild<gtk::Button>,
        pub device: OnceCell<Device>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for MirageDeviceRow {
        const NAME: &'static str = "MirageDeviceRow";
        type Type = super::MirageDeviceRow;
        type ParentType = adw::ActionRow;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for MirageDeviceRow {}
    impl WidgetImpl for MirageDeviceRow {}
    impl ListBoxRowImpl for MirageDeviceRow {}
    impl PreferencesRowImpl for MirageDeviceRow {}
    impl ActionRowImpl for MirageDeviceRow {}
}

glib::wrapper! {
    pub struct MirageDeviceRow(ObjectSubclass<imp::MirageDeviceRow>)
        @extends gtk::Widget, gtk::ListBoxRow, adw::PreferencesRow, adw::ActionRow,
        @implements gtk::Accessible, gtk::Actionable, gtk::Buildable, gtk::ConstraintTarget;
}

impl MirageDeviceRow {
    pub fn new(device: &Device) -> Self {
        let row: Self = glib::Object::new();
        row.imp().device.set(device.clone()).unwrap();

        let serial = device.serial().to_variant();
        row.imp()
            .mirror_button
            .set_action_target_value(Some(&serial));
        row.imp()
            .record_button
            .set_action_target_value(Some(&serial));

        device.connect_notify_local(
            None,
            clone!(
                #[weak]
                row,
                move |_, _| row.update()
            ),
        );
        row.update();
        row
    }

    fn update(&self) {
        let imp = self.imp();
        let device = imp.device.get().unwrap();
        let online = device.is_online();
        let activity = device.activity();

        self.set_title(&device.name());
        self.set_subtitle(&subtitle(device));

        let (icon, connection) = if device.wireless() {
            ("network-wireless-symbolic", gettext("Wi-Fi"))
        } else {
            ("mirage-usb-symbolic", gettext("USB"))
        };
        imp.connection_icon.set_icon_name(Some(icon));
        imp.connection_icon.set_tooltip_text(Some(&connection));

        let (mirror_icon, mirror_label, mirror_action) = match activity {
            Activity::Mirroring => (
                "media-playback-stop-symbolic",
                gettext("Stop Mirroring"),
                "win.stop",
            ),
            _ => (
                "screen-shared-symbolic",
                gettext("Mirror Screen"),
                "win.mirror",
            ),
        };
        set_button(
            &imp.mirror_button,
            mirror_icon,
            &mirror_label,
            mirror_action,
            online && activity != Activity::Recording,
        );

        let (record_icon, record_label, record_action) = match activity {
            Activity::Recording => (
                "media-playback-stop-symbolic",
                gettext("Stop Recording"),
                "win.stop",
            ),
            _ => (
                "media-record-symbolic",
                gettext("Record Screen"),
                "win.record",
            ),
        };
        set_button(
            &imp.record_button,
            record_icon,
            &record_label,
            record_action,
            online && activity != Activity::Mirroring,
        );
        if activity == Activity::Recording {
            imp.record_button.add_css_class("recording");
        } else {
            imp.record_button.remove_css_class("recording");
        }

        self.set_activatable(online && activity != Activity::Recording);
    }
}

fn set_button(button: &gtk::Button, icon: &str, label: &str, action: &str, sensitive: bool) {
    button.set_icon_name(icon);
    button.set_tooltip_text(Some(label));
    button.update_property(&[gtk::accessible::Property::Label(label)]);
    button.set_action_name(Some(action));
    button.set_sensitive(sensitive);
}

fn subtitle(device: &Device) -> String {
    match device.state() {
        DeviceState::Online => {
            let connection = if device.wireless() {
                match device.network_host() {
                    Some(host) => gettext_f("Wi-Fi · {address}", &[("address", &host)]),
                    None => gettext("Wi-Fi"),
                }
            } else {
                gettext("USB")
            };
            match device.activity() {
                Activity::Idle => connection,
                Activity::Mirroring => {
                    gettext_f("{connection} · Mirroring", &[("connection", &connection)])
                }
                Activity::Recording => {
                    gettext_f("{connection} · Recording", &[("connection", &connection)])
                }
            }
        }
        DeviceState::Unauthorized => gettext("Allow USB debugging on the device to continue"),
        DeviceState::NoPermissions => gettext(
            "The device cannot be accessed. Installing the Android udev rules on this computer may fix this.",
        ),
        DeviceState::Connecting => gettext("Connecting…"),
        DeviceState::Offline => gettext("Offline"),
        DeviceState::Unavailable => {
            // Translators: {state} is a mode such as "recovery" or "bootloader"
            gettext_f(
                "Unavailable in {state} mode",
                &[("state", &device.raw_state())],
            )
        }
    }
}
