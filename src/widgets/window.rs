use std::cell::{Cell, OnceCell, RefCell};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use adw::prelude::*;
use adw::subclass::prelude::*;
use gettextrs::gettext;
use gtk::glib::{clone, closure_local};
use gtk::{gio, glib};

use super::{MirageDeviceRow, MiragePairingDialog};
use crate::adb::discovery::{self, Discovery, Service};
use crate::adb::{AdbClient, TrackerEvent, TrackerHandle, spawn_tracker};
use crate::application::MirageApplication;
use crate::config::{self, is_devel};
use crate::device::{Activity, Device};
use crate::i18n::{gettext_f, ngettext_f};
use crate::recordings;
use crate::scrcpy::{Options, Outcome, Recording, Session};

/// How long scrcpy may take to finish the recordings when closing.
const STOP_TIMEOUT: Duration = Duration::from_secs(5);

mod imp {
    use super::*;

    #[derive(Default, gtk::CompositeTemplate)]
    #[template(resource = "/tech/vitainside/mirage/ui/window.ui")]
    pub struct MirageWindow {
        #[template_child]
        pub toast_overlay: TemplateChild<adw::ToastOverlay>,
        #[template_child]
        pub stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub error_page: TemplateChild<adw::StatusPage>,
        #[template_child]
        pub device_list: TemplateChild<gtk::ListBox>,

        pub settings: OnceCell<gio::Settings>,
        pub client: AdbClient,
        pub devices: OnceCell<gio::ListStore>,
        pub tracker: OnceCell<TrackerHandle>,
        pub sessions: RefCell<HashMap<String, Session>>,
        pub closing: Cell<bool>,

        pub discovery: OnceCell<Option<Rc<Discovery>>>,
        /// Devices on the network with wireless debugging on, by mDNS full name.
        pub connect_services: RefCell<HashMap<String, Service>>,
        /// Addresses that were already tried, so unpaired devices are not retried.
        pub connect_attempts: RefCell<HashSet<String>>,
        /// Wireless devices are tried again once a new server lists its devices.
        pub reconnect_pending: Cell<bool>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for MirageWindow {
        const NAME: &'static str = "MirageWindow";
        type Type = super::MirageWindow;
        type ParentType = adw::ApplicationWindow;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();

            klass.install_action(
                "win.mirror",
                Some(glib::VariantTy::STRING),
                |window, _, serial| {
                    if let Some(serial) = serial.and_then(|serial| serial.str()) {
                        window.start_session(serial, false);
                    }
                },
            );
            klass.install_action(
                "win.record",
                Some(glib::VariantTy::STRING),
                |window, _, serial| {
                    if let Some(serial) = serial.and_then(|serial| serial.str()) {
                        window.start_session(serial, true);
                    }
                },
            );
            klass.install_action(
                "win.stop",
                Some(glib::VariantTy::STRING),
                |window, _, serial| {
                    let Some(serial) = serial.and_then(|serial| serial.str()) else {
                        return;
                    };
                    if let Some(session) = window.imp().sessions.borrow().get(serial) {
                        session.stop();
                    }
                },
            );
            klass.install_action(
                "win.show-recording",
                Some(glib::VariantTy::STRING),
                |window, _, path| {
                    if let Some(path) = path.and_then(|path| path.str()) {
                        window.show_recording(path);
                    }
                },
            );
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for MirageWindow {
        fn constructed(&self) {
            self.parent_constructed();
            let obj = self.obj();

            if is_devel() {
                obj.add_css_class("devel");
            }

            let settings = config::settings();
            obj.set_default_size(settings.int("window-width"), settings.int("window-height"));
            if settings.boolean("is-maximized") {
                obj.maximize();
            }
            self.settings.set(settings).unwrap();

            obj.setup_actions();
            obj.setup_device_list();
            obj.setup_tracker();
            obj.setup_discovery();
        }
    }

    impl WidgetImpl for MirageWindow {}

    impl WindowImpl for MirageWindow {
        fn close_request(&self) -> glib::Propagation {
            let obj = self.obj();
            obj.save_window_state();

            let sessions = self.sessions.borrow().len();
            if sessions == 0 {
                if let Some(tracker) = self.tracker.get() {
                    tracker.shutdown();
                }
                return self.parent_close_request();
            }
            if !self.closing.get() {
                obj.confirm_close(sessions as u32);
            }
            glib::Propagation::Stop
        }
    }

    impl ApplicationWindowImpl for MirageWindow {}
    impl AdwApplicationWindowImpl for MirageWindow {}
}

glib::wrapper! {
    pub struct MirageWindow(ObjectSubclass<imp::MirageWindow>)
        @extends gtk::Widget, gtk::Window, gtk::ApplicationWindow, adw::ApplicationWindow,
        @implements gio::ActionGroup, gio::ActionMap, gtk::Accessible, gtk::Buildable,
                    gtk::ConstraintTarget, gtk::Native, gtk::Root, gtk::ShortcutManager;
}

impl MirageWindow {
    pub fn new(app: &MirageApplication) -> Self {
        glib::Object::builder().property("application", app).build()
    }

    fn settings(&self) -> &gio::Settings {
        self.imp().settings.get().unwrap()
    }

    fn devices(&self) -> &gio::ListStore {
        self.imp().devices.get().unwrap()
    }

    fn save_window_state(&self) {
        let settings = self.settings();
        let (width, height) = self.default_size();
        let _ = settings.set_int("window-width", width);
        let _ = settings.set_int("window-height", height);
        let _ = settings.set_boolean("is-maximized", self.is_maximized());
    }

    fn toast(&self, toast: adw::Toast) {
        self.imp().toast_overlay.add_toast(toast);
    }

    fn device(&self, serial: &str) -> Option<Device> {
        self.devices()
            .iter::<Device>()
            .flatten()
            .find(|device| device.serial() == serial)
    }

    /// Actions without a parameter, also exported on D-Bus.
    fn setup_actions(&self) {
        let pair_device = gio::ActionEntry::builder("pair-device")
            .activate(|window: &Self, _, _| window.show_pairing_dialog())
            .build();
        let retry_adb = gio::ActionEntry::builder("retry-adb")
            .activate(|window: &Self, _, _| {
                window.imp().stack.set_visible_child_name("loading");
                if let Some(tracker) = window.imp().tracker.get() {
                    tracker.retry();
                }
            })
            .build();
        let rescan = gio::ActionEntry::builder("rescan")
            .activate(|window: &Self, _, _| window.rescan())
            .build();
        self.add_action_entries([pair_device, retry_adb, rescan]);
    }

    // Device list

    fn setup_device_list(&self) {
        let devices = gio::ListStore::new::<Device>();
        self.imp().device_list.bind_model(Some(&devices), |item| {
            let device = item.downcast_ref::<Device>().unwrap();
            MirageDeviceRow::new(device).upcast()
        });
        self.imp().devices.set(devices).unwrap();
        self.imp().stack.set_visible_child_name("loading");
    }

    fn setup_tracker(&self) {
        let (tracker, receiver) = spawn_tracker(self.imp().client.clone());
        self.imp().tracker.set(tracker).ok().unwrap();

        let window = self.downgrade();
        glib::spawn_future_local(async move {
            while let Ok(event) = receiver.recv().await {
                let Some(window) = window.upgrade() else {
                    break;
                };
                window.handle_tracker_event(event);
            }
        });
    }

    fn handle_tracker_event(&self, event: TrackerEvent) {
        let imp = self.imp();
        match event {
            TrackerEvent::Devices(list) => {
                let devices = self.devices();

                // Remove the devices that are gone, keeping the order of the others
                for position in (0..devices.n_items()).rev() {
                    let device = devices.item(position).and_downcast::<Device>().unwrap();
                    if !list.iter().any(|info| info.serial == device.serial()) {
                        devices.remove(position);
                    }
                }

                for info in &list {
                    let device = match self.device(&info.serial) {
                        Some(device) => {
                            device.update(info);
                            device
                        }
                        None => {
                            let device = Device::new(info);
                            devices.append(&device);
                            device
                        }
                    };
                    if device.is_online() && device.take_name_request() {
                        self.fetch_device_name(&device);
                    }
                }

                if imp.reconnect_pending.take() {
                    self.reconnect_wireless();
                }

                let page = if devices.n_items() == 0 {
                    "empty"
                } else {
                    "devices"
                };
                imp.stack.set_visible_child_name(page);
            }
            TrackerEvent::ServerStarted => imp.reconnect_pending.set(true),
            TrackerEvent::Unavailable(message) => {
                self.devices().remove_all();
                imp.error_page.set_description(Some(&message));
                imp.stack.set_visible_child_name("error");
            }
        }
    }

    /// Looks for devices again. The ADB server in the Flatpak sandbox does not
    /// see USB devices plugged in after it started, so a server started by
    /// Mirage is restarted to scan them again. Mirroring sessions would stop
    /// with it, so it is left running while there are any.
    fn rescan(&self) {
        let imp = self.imp();
        let Some(tracker) = imp.tracker.get() else {
            return;
        };
        if !tracker.started_server() {
            self.reconnect_wireless();
            return;
        }
        if !imp.sessions.borrow().is_empty() {
            self.reconnect_wireless();
            self.toast(adw::Toast::new(&gettext(
                "Stop mirroring to look for new USB devices",
            )));
            return;
        }

        tracing::info!("Restarting the ADB server to rescan devices");
        imp.stack.set_visible_child_name("loading");
        let client = imp.client.clone();
        glib::spawn_future_local(async move {
            if let Ok(Err(error)) = gio::spawn_blocking(move || client.kill_server()).await {
                tracing::debug!("Could not stop the ADB server: {error}");
            }
        });
    }

    /// Tries again the wireless devices on the network that are not connected.
    fn reconnect_wireless(&self) {
        let services: Vec<Service> = self
            .imp()
            .connect_services
            .borrow()
            .values()
            .cloned()
            .collect();
        self.imp().connect_attempts.borrow_mut().clear();
        for service in services {
            self.auto_connect(&service);
        }
    }

    /// Replaces the model name with the name the user gave to the device.
    fn fetch_device_name(&self, device: &Device) {
        let client = self.imp().client.clone();
        let serial = device.serial();
        let device = device.downgrade();
        glib::spawn_future_local(async move {
            let result = gio::spawn_blocking(move || {
                client.shell(&serial, "settings get global device_name")
            })
            .await;
            let Some(device) = device.upgrade() else {
                return;
            };
            match result {
                Ok(Ok(output)) => {
                    let name = output.trim();
                    if !name.is_empty() && name != "null" {
                        device.set_custom_name(Some(name.to_owned()));
                    }
                }
                Ok(Err(error)) => {
                    tracing::debug!("Could not get the name of {}: {error}", device.serial());
                }
                Err(_) => {}
            }
        });
    }

    // Wireless devices

    fn setup_discovery(&self) {
        let discovery = match Discovery::new() {
            Ok(discovery) => Rc::new(discovery),
            Err(error) => {
                tracing::warn!("Network discovery is not available: {error}");
                self.imp().discovery.set(None).ok().unwrap();
                return;
            }
        };

        let window = self.downgrade();
        let result = discovery.browse(discovery::CONNECT_SERVICE, move |event| {
            if let Some(window) = window.upgrade() {
                window.handle_connect_service(event);
            }
        });
        if let Err(error) = result {
            tracing::warn!("Could not look for wireless devices: {error}");
        }
        self.imp().discovery.set(Some(discovery)).ok().unwrap();
    }

    fn handle_connect_service(&self, event: discovery::Event) {
        let imp = self.imp();
        match event {
            discovery::Event::Resolved(service) => {
                tracing::debug!("Found {} at {}", service.instance, service.address);
                imp.connect_services
                    .borrow_mut()
                    .insert(service.fullname.clone(), service.clone());
                self.auto_connect(&service);
            }
            discovery::Event::Removed { fullname } => {
                if let Some(service) = imp.connect_services.borrow_mut().remove(&fullname) {
                    imp.connect_attempts.borrow_mut().remove(&service.address);
                }
            }
        }
    }

    fn is_connected(&self, service: &Service) -> bool {
        let instance_serial = format!("{}.", service.instance);
        let ip = service.ip.to_string();
        self.devices().iter::<Device>().flatten().any(|device| {
            let serial = device.serial();
            serial == service.address
                || serial.starts_with(&instance_serial)
                || device.network_host().is_some_and(|host| host == ip)
        })
    }

    /// Connects to paired devices that appear on the network. Connecting to a
    /// device that is not paired fails, so every address is only tried once.
    fn auto_connect(&self, service: &Service) {
        if self.is_connected(service)
            || !self
                .imp()
                .connect_attempts
                .borrow_mut()
                .insert(service.address.clone())
        {
            return;
        }

        let client = self.imp().client.clone();
        let address = service.address.clone();
        glib::spawn_future_local(async move {
            let result = gio::spawn_blocking({
                let address = address.clone();
                move || client.connect_device(&address)
            })
            .await;
            match result {
                Ok(Ok(reply)) => tracing::info!("{reply}"),
                Ok(Err(error)) => tracing::info!("Could not connect to {address}: {error}"),
                Err(_) => {}
            }
        });
    }

    /// Connects to a device right after pairing with it.
    fn on_device_paired(&self, host: &str) {
        let imp = self.imp();
        let services: Vec<Service> = imp
            .connect_services
            .borrow()
            .values()
            .filter(|service| service.ip.to_string() == host)
            .cloned()
            .collect();
        if services.is_empty() {
            tracing::debug!("No wireless debugging service found for {host} yet");
        }
        for service in services {
            imp.connect_attempts.borrow_mut().remove(&service.address);
            self.auto_connect(&service);
        }
        self.toast(adw::Toast::new(&gettext("Device paired")));
    }

    fn show_pairing_dialog(&self) {
        let discovery = self.imp().discovery.get().cloned().flatten();
        let dialog = MiragePairingDialog::new(self.imp().client.clone(), discovery);
        dialog.connect_closure(
            "paired",
            false,
            closure_local!(
                #[weak(rename_to = window)]
                self,
                move |_: MiragePairingDialog, host: String| window.on_device_paired(&host)
            ),
        );
        dialog.present(Some(self));
    }

    // Mirroring sessions

    fn start_session(&self, serial: &str, record: bool) {
        let Some(device) = self.device(serial) else {
            return;
        };
        if !device.is_online() || self.imp().sessions.borrow().contains_key(serial) {
            return;
        }

        let settings = self.settings();
        let name = device.name();
        let recording = if record {
            let format = settings.string("record-format").to_string();
            match recordings::new_recording_path(&name, &format) {
                Ok(path) => Some(Recording { path, format }),
                Err(error) => {
                    self.show_error(&gettext("Could Not Start Recording"), &error.to_string());
                    return;
                }
            }
        } else {
            None
        };

        let args = Options::from_settings(settings).to_args(serial, &name, recording.as_ref());
        let recording_path = recording.map(|recording| recording.path);
        let serial = serial.to_owned();
        let on_exit = clone!(
            #[weak(rename_to = window)]
            self,
            #[strong]
            serial,
            move |outcome| window.on_session_finished(&serial, &name, recording_path, outcome)
        );

        match Session::launch(&args, on_exit) {
            Ok(session) => {
                self.imp().sessions.borrow_mut().insert(serial, session);
                device.set_activity(if record {
                    Activity::Recording
                } else {
                    Activity::Mirroring
                });
            }
            Err(error) => {
                self.show_error(&gettext("Could Not Start Mirroring"), error.message());
            }
        }
    }

    fn on_session_finished(
        &self,
        serial: &str,
        name: &str,
        recording: Option<PathBuf>,
        outcome: Outcome,
    ) {
        let imp = self.imp();
        imp.sessions.borrow_mut().remove(serial);
        if let Some(device) = self.device(serial) {
            device.set_activity(Activity::Idle);
        }

        match outcome {
            Outcome::Finished => {}
            Outcome::Disconnected => {
                self.toast(adw::Toast::new(&gettext_f(
                    "{device} was disconnected",
                    &[("device", name)],
                )));
            }
            Outcome::Failed(message) => {
                let heading = if recording.is_some() {
                    gettext_f("Could Not Record {device}", &[("device", name)])
                } else {
                    gettext_f("Could Not Mirror {device}", &[("device", name)])
                };
                self.show_error(&heading, &message);
            }
        }

        if let Some(path) = recording.filter(|path| {
            path.metadata()
                .is_ok_and(|metadata| metadata.is_file() && metadata.len() > 0)
        }) {
            let toast = adw::Toast::builder()
                .title(gettext("Recording saved"))
                .button_label(gettext("_Show"))
                .action_name("win.show-recording")
                .action_target(&path.to_string_lossy().to_variant())
                .build();
            self.toast(toast);
        }

        if imp.closing.get() && imp.sessions.borrow().is_empty() {
            self.close();
        }
    }

    fn show_recording(&self, path: &str) {
        let file = gio::File::for_path(path);
        gtk::FileLauncher::new(Some(&file)).open_containing_folder(
            Some(self),
            gio::Cancellable::NONE,
            |result| {
                if let Err(error) = result {
                    tracing::warn!("Could not show the recording: {error}");
                }
            },
        );
    }

    fn show_error(&self, heading: &str, body: &str) {
        let dialog = adw::AlertDialog::new(Some(heading), Some(body));
        dialog.add_response("close", &gettext("_Close"));
        dialog.set_body_use_markup(false);
        dialog.present(Some(self));
    }

    fn confirm_close(&self, sessions: u32) {
        let dialog = adw::AlertDialog::new(
            Some(&gettext("Stop Mirroring?")),
            Some(&ngettext_f(
                "Mirroring and recording will stop on {count} device.",
                "Mirroring and recording will stop on {count} devices.",
                sessions,
                &[("count", &sessions.to_string())],
            )),
        );
        dialog.add_response("cancel", &gettext("_Cancel"));
        dialog.add_response("stop", &gettext("_Stop and Close"));
        dialog.set_response_appearance("stop", adw::ResponseAppearance::Destructive);
        dialog.set_default_response(Some("cancel"));
        dialog.set_close_response("cancel");
        dialog.connect_response(
            Some("stop"),
            clone!(
                #[weak(rename_to = window)]
                self,
                move |_, _| window.stop_all_and_close()
            ),
        );
        dialog.present(Some(self));
    }

    fn stop_all_and_close(&self) {
        let imp = self.imp();
        imp.closing.set(true);
        self.set_sensitive(false);
        for session in imp.sessions.borrow().values() {
            session.stop();
        }
        glib::timeout_add_local_once(
            STOP_TIMEOUT,
            clone!(
                #[weak(rename_to = window)]
                self,
                move || {
                    for session in window.imp().sessions.borrow().values() {
                        session.kill();
                    }
                }
            ),
        );
    }
}
