use adw::prelude::*;
use adw::subclass::prelude::*;
use gettextrs::gettext;
use gtk::{gio, glib};

use crate::config::{APP_ID, RESOURCE_BASE_PATH, VERSION};
use crate::widgets::{MiragePreferencesDialog, MirageWindow};

mod imp {
    use super::*;

    #[derive(Debug, Default)]
    pub struct MirageApplication;

    #[glib::object_subclass]
    impl ObjectSubclass for MirageApplication {
        const NAME: &'static str = "MirageApplication";
        type Type = super::MirageApplication;
        type ParentType = adw::Application;
    }

    impl ObjectImpl for MirageApplication {}

    impl ApplicationImpl for MirageApplication {
        fn startup(&self) {
            self.parent_startup();
            let app = self.obj();
            gtk::Window::set_default_icon_name(APP_ID);
            app.setup_actions();
            app.setup_accels();
        }

        fn activate(&self) {
            let app = self.obj();
            if let Some(window) = app.active_window() {
                window.present();
                return;
            }
            MirageWindow::new(&app).present();
        }
    }

    impl GtkApplicationImpl for MirageApplication {}
    impl AdwApplicationImpl for MirageApplication {}
}

glib::wrapper! {
    pub struct MirageApplication(ObjectSubclass<imp::MirageApplication>)
        @extends gio::Application, gtk::Application, adw::Application,
        @implements gio::ActionGroup, gio::ActionMap;
}

impl MirageApplication {
    pub fn new() -> Self {
        glib::Object::builder()
            .property("application-id", APP_ID)
            .property("flags", gio::ApplicationFlags::empty())
            .property("resource-base-path", RESOURCE_BASE_PATH)
            .build()
    }

    fn setup_actions(&self) {
        let quit = gio::ActionEntry::builder("quit")
            .activate(|app: &Self, _, _| {
                // Closing goes through the confirmation of each window
                for window in app.windows() {
                    window.close();
                }
            })
            .build();

        let preferences = gio::ActionEntry::builder("preferences")
            .activate(|app: &Self, _, _| {
                MiragePreferencesDialog::new().present(app.active_window().as_ref());
            })
            .build();

        let about = gio::ActionEntry::builder("about")
            .activate(|app: &Self, _, _| app.show_about_dialog())
            .build();

        self.add_action_entries([quit, preferences, about]);
    }

    fn setup_accels(&self) {
        self.set_accels_for_action("app.quit", &["<Control>q"]);
        self.set_accels_for_action("app.preferences", &["<Control>comma"]);
        self.set_accels_for_action("window.close", &["<Control>w"]);
        self.set_accels_for_action("win.pair-device", &["<Control>n"]);
    }

    fn show_about_dialog(&self) {
        let developer = "Martín Nehuen Quiroga Torres";
        let dialog = adw::AboutDialog::builder()
            .application_name(gettext("Mirage"))
            .application_icon(APP_ID)
            .developer_name(developer)
            .developers([developer])
            .version(VERSION)
            .website("https://github.com/cr3ativec0mmons/mirage")
            .issue_url("https://github.com/cr3ativec0mmons/mirage/issues")
            .license_type(gtk::License::Gpl30)
            // Translators: Replace "translator-credits" with your name and email, one translator per line
            .translator_credits(gettext("translator-credits"))
            .build();
        dialog.add_legal_section(
            "scrcpy",
            Some("© 2018 Genymobile\n© 2018–2026 Romain Vimont"),
            gtk::License::Apache20,
            None,
        );
        dialog.add_legal_section(
            "Android Debug Bridge",
            Some("© The Android Open Source Project"),
            gtk::License::Apache20,
            None,
        );
        dialog.present(self.active_window().as_ref());
    }
}
