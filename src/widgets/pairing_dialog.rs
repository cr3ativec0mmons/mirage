use std::cell::{Cell, OnceCell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::OnceLock;

use adw::prelude::*;
use adw::subclass::prelude::*;
use gettextrs::gettext;
use gtk::glib::clone;
use gtk::{gio, glib};

use super::qr_code::QrCodePaintable;
use crate::adb::discovery::{self, Discovery, Service};
use crate::adb::{AdbClient, Error};
use crate::i18n::gettext_f;

/// Characters for the QR pairing secrets. 32 of them, so a random byte maps to
/// one without bias, and none need escaping in the QR payload.
const ALPHABET: &[u8; 32] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";

/// The secrets of a QR code pairing. Android advertises a pairing service
/// with `name` once the code is scanned, and expects `password` from us.
#[derive(Debug, Clone)]
pub(super) struct QrPairing {
    name: String,
    password: String,
}

impl QrPairing {
    fn generate() -> Self {
        Self {
            name: format!("mirage-{}", random_string(8)),
            password: random_string(12),
        }
    }

    fn payload(&self) -> String {
        format!("WIFI:T:ADB;S:{};P:{};;", self.name, self.password)
    }
}

fn random_string(len: usize) -> String {
    let mut bytes = vec![0; len];
    getrandom::fill(&mut bytes).expect("Could not generate random data");
    bytes
        .into_iter()
        .map(|byte| ALPHABET[usize::from(byte) % ALPHABET.len()] as char)
        .collect()
}

/// Splits `host:port` and removes the brackets of IPv6 addresses.
fn host_of(address: &str) -> &str {
    address
        .rsplit_once(':')
        .map_or(address, |(host, _)| host)
        .trim_start_matches('[')
        .trim_end_matches(']')
}

fn is_valid_address(address: &str) -> bool {
    address
        .rsplit_once(':')
        .is_some_and(|(host, port)| !host.is_empty() && port.parse::<u16>().is_ok())
}

mod imp {
    use super::*;

    #[derive(Default, gtk::CompositeTemplate)]
    #[template(resource = "/tech/vitainside/mirage/ui/pairing-dialog.ui")]
    pub struct MiragePairingDialog {
        #[template_child]
        pub toast_overlay: TemplateChild<adw::ToastOverlay>,
        #[template_child]
        pub navigation_view: TemplateChild<adw::NavigationView>,
        #[template_child]
        pub method_toggle: TemplateChild<adw::ToggleGroup>,
        #[template_child]
        pub qr_picture: TemplateChild<gtk::Picture>,
        #[template_child]
        pub qr_status_stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub qr_status_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub qr_error_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub service_list: TemplateChild<gtk::ListBox>,
        #[template_child]
        pub address_row: TemplateChild<adw::EntryRow>,
        #[template_child]
        pub code_row: TemplateChild<adw::EntryRow>,
        #[template_child]
        pub pair_button: TemplateChild<adw::ButtonRow>,

        pub client: OnceCell<AdbClient>,
        pub discovery: RefCell<Option<Rc<Discovery>>>,
        pub(super) qr: RefCell<Option<QrPairing>>,
        /// Whether pairing was attempted with the current QR code.
        pub qr_used: Cell<bool>,
        /// Devices showing a pairing code, by mDNS full name.
        pub services: RefCell<HashMap<String, (Service, adw::ActionRow)>>,
        pub busy: Cell<bool>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for MiragePairingDialog {
        const NAME: &'static str = "MiragePairingDialog";
        type Type = super::MiragePairingDialog;
        type ParentType = adw::Dialog;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();

            klass.install_action("pairing.restart-qr", None, |dialog, _, _| {
                dialog.restart_qr();
            });
            klass.install_action("pairing.enter-manually", None, |dialog, _, _| {
                dialog.show_code_entry(None);
            });
            klass.install_action("pairing.pair-with-code", None, |dialog, _, _| {
                dialog.pair_with_code();
            });
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for MiragePairingDialog {
        fn signals() -> &'static [glib::subclass::Signal] {
            static SIGNALS: OnceLock<Vec<glib::subclass::Signal>> = OnceLock::new();
            SIGNALS.get_or_init(|| {
                // Emitted with the host of the device after a successful pairing
                vec![
                    glib::subclass::Signal::builder("paired")
                        .param_types([String::static_type()])
                        .build(),
                ]
            })
        }

        fn constructed(&self) {
            self.parent_constructed();
            let obj = self.obj();

            self.address_row.connect_changed(clone!(
                #[weak]
                obj,
                move |_| obj.update_pair_action()
            ));
            self.code_row.connect_changed(clone!(
                #[weak]
                obj,
                move |_| obj.update_pair_action()
            ));
            self.code_row.connect_entry_activated(clone!(
                #[weak]
                obj,
                move |_| obj.pair_with_code()
            ));
            obj.update_pair_action();
        }
    }

    impl WidgetImpl for MiragePairingDialog {}

    impl AdwDialogImpl for MiragePairingDialog {
        fn closed(&self) {
            if let Some(discovery) = self.discovery.borrow().as_ref() {
                discovery.stop_browse(discovery::PAIRING_SERVICE);
            }
            self.parent_closed();
        }
    }
}

glib::wrapper! {
    pub struct MiragePairingDialog(ObjectSubclass<imp::MiragePairingDialog>)
        @extends gtk::Widget, adw::Dialog,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::ShortcutManager;
}

impl MiragePairingDialog {
    pub fn new(client: AdbClient, discovery: Option<Rc<Discovery>>) -> Self {
        let dialog: Self = glib::Object::new();
        let imp = dialog.imp();
        imp.client.set(client).unwrap();

        match discovery {
            Some(discovery) => {
                let weak = dialog.downgrade();
                let result = discovery.browse(discovery::PAIRING_SERVICE, move |event| {
                    if let Some(dialog) = weak.upgrade() {
                        dialog.handle_pairing_service(event);
                    }
                });
                imp.discovery.replace(Some(discovery));
                match result {
                    Ok(()) => dialog.restart_qr(),
                    Err(error) => dialog.discovery_unavailable(&error.to_string()),
                }
            }
            None => dialog.discovery_unavailable(""),
        }

        dialog
    }

    fn client(&self) -> AdbClient {
        self.imp().client.get().unwrap().clone()
    }

    /// Without mDNS, only pairing with a manually entered address works.
    fn discovery_unavailable(&self, details: &str) {
        let imp = self.imp();
        tracing::warn!("Pairing discovery is not available: {details}");
        imp.method_toggle.set_active_name(Some("code"));
        imp.method_toggle.set_sensitive(false);
        imp.service_list.set_visible(false);
    }

    // QR code

    fn restart_qr(&self) {
        let imp = self.imp();
        let pairing = QrPairing::generate();
        let paintable = QrCodePaintable::new(&pairing.payload());
        imp.qr_picture.set_paintable(paintable.as_ref());
        imp.qr.replace(Some(pairing));
        imp.qr_used.set(false);
        imp.qr_status_label
            .set_label(&gettext("Waiting for the device…"));
        imp.qr_status_stack.set_visible_child_name("waiting");
    }

    fn pair_with_qr(&self, service: &Service) {
        let imp = self.imp();
        let Some(pairing) = imp.qr.borrow().clone() else {
            return;
        };
        if imp.busy.get() || imp.qr_used.replace(true) {
            return;
        }
        imp.qr_status_label.set_label(&gettext("Pairing…"));

        let service = service.clone();
        self.pair(
            service.address.clone(),
            pairing.password,
            clone!(
                #[weak(rename_to = dialog)]
                self,
                move |result| match result {
                    Ok(()) => dialog.finish(&service.ip.to_string()),
                    Err(message) => {
                        let imp = dialog.imp();
                        imp.qr_error_label.set_label(&gettext_f(
                            "Pairing failed: {error}",
                            &[("error", &message)],
                        ));
                        imp.qr_status_stack.set_visible_child_name("error");
                    }
                }
            ),
        );
    }

    // Pairing code

    fn handle_pairing_service(&self, event: discovery::Event) {
        let imp = self.imp();
        match event {
            discovery::Event::Resolved(service) => {
                let is_qr_device = imp
                    .qr
                    .borrow()
                    .as_ref()
                    .is_some_and(|pairing| pairing.name == service.instance);
                if is_qr_device {
                    self.pair_with_qr(&service);
                    return;
                }
                self.add_service_row(service);
            }
            discovery::Event::Removed { fullname } => {
                if let Some((_, row)) = imp.services.borrow_mut().remove(&fullname) {
                    imp.service_list.remove(&row);
                }
            }
        }
    }

    fn add_service_row(&self, service: Service) {
        let imp = self.imp();
        if let Some((_, row)) = imp.services.borrow_mut().remove(&service.fullname) {
            imp.service_list.remove(&row);
        }

        let row = adw::ActionRow::builder()
            .title(&service.address)
            .subtitle(&service.instance)
            .use_markup(false)
            .activatable(true)
            .build();
        row.add_suffix(&gtk::Image::from_icon_name("go-next-symbolic"));
        let address = service.address.clone();
        row.connect_activated(clone!(
            #[weak(rename_to = dialog)]
            self,
            move |_| dialog.show_code_entry(Some(&address))
        ));

        imp.service_list.append(&row);
        imp.services
            .borrow_mut()
            .insert(service.fullname.clone(), (service, row));
    }

    fn show_code_entry(&self, address: Option<&str>) {
        let imp = self.imp();
        imp.address_row.set_text(address.unwrap_or_default());
        imp.code_row.set_text("");
        imp.navigation_view.push_by_tag("code-entry");
        if address.is_some() {
            imp.code_row.grab_focus();
        } else {
            imp.address_row.grab_focus();
        }
    }

    fn update_pair_action(&self) {
        let imp = self.imp();
        let valid = is_valid_address(imp.address_row.text().trim())
            && !imp.code_row.text().trim().is_empty();
        self.action_set_enabled("pairing.pair-with-code", valid && !imp.busy.get());
    }

    fn pair_with_code(&self) {
        let imp = self.imp();
        let address = imp.address_row.text().trim().to_owned();
        let code = imp.code_row.text().trim().to_owned();
        if imp.busy.get() || !is_valid_address(&address) || code.is_empty() {
            return;
        }
        imp.pair_button.set_title(&gettext("Pairing…"));

        let host = host_of(&address).to_owned();
        self.pair(
            address,
            code,
            clone!(
                #[weak(rename_to = dialog)]
                self,
                move |result| {
                    let imp = dialog.imp();
                    imp.pair_button.set_title(&gettext("_Pair"));
                    match result {
                        Ok(()) => dialog.finish(&host),
                        Err(message) => {
                            imp.toast_overlay.add_toast(adw::Toast::new(&gettext_f(
                                "Pairing failed: {error}",
                                &[("error", &message)],
                            )));
                            imp.code_row.grab_focus();
                        }
                    }
                }
            ),
        );
    }

    // Pairing

    fn pair(&self, address: String, code: String, done: impl FnOnce(Result<(), String>) + 'static) {
        let imp = self.imp();
        imp.busy.set(true);
        self.update_pair_action();

        let client = self.client();
        let dialog = self.downgrade();
        glib::spawn_future_local(async move {
            let result = gio::spawn_blocking(move || client.pair(&address, &code))
                .await
                .unwrap_or_else(|_| Err(Error::Protocol("pairing thread panicked".to_owned())));
            let Some(dialog) = dialog.upgrade() else {
                return;
            };
            dialog.imp().busy.set(false);
            dialog.update_pair_action();
            match result {
                Ok(reply) => {
                    tracing::info!("{reply}");
                    done(Ok(()));
                }
                Err(error) => done(Err(error.to_string())),
            }
        });
    }

    fn finish(&self, host: &str) {
        self.emit_by_name::<()>("paired", &[&host.to_owned()]);
        self.close();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qr_payload_format() {
        let pairing = QrPairing::generate();
        assert!(pairing.name.starts_with("mirage-"));
        assert_eq!(pairing.password.len(), 12);
        assert_eq!(
            pairing.payload(),
            format!("WIFI:T:ADB;S:{};P:{};;", pairing.name, pairing.password)
        );
        assert!(
            pairing
                .password
                .bytes()
                .all(|byte| ALPHABET.contains(&byte))
        );
    }

    #[test]
    fn parses_addresses() {
        assert_eq!(host_of("192.168.1.20:37099"), "192.168.1.20");
        assert_eq!(host_of("[2001:db8::5]:5555"), "2001:db8::5");
        assert!(is_valid_address("192.168.1.20:37099"));
        assert!(is_valid_address("[2001:db8::5]:5555"));
        assert!(!is_valid_address("192.168.1.20"));
        assert!(!is_valid_address(":5555"));
        assert!(!is_valid_address("phone:port"));
    }
}
