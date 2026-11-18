mod adb;
mod application;
mod config;
mod device;
mod i18n;
mod recordings;
mod scrcpy;
mod widgets;

use gettextrs::{LocaleCategory, gettext};
use gtk::prelude::*;
use gtk::{gio, glib};

use self::application::MirageApplication;
use self::config::{GETTEXT_PACKAGE, LOCALEDIR};

fn main() -> glib::ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_env("MIRAGE_LOG")
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("mirage=info")),
        )
        .init();

    // SAFETY: No other thread is running yet that could read the locale
    unsafe {
        gettextrs::setlocale(LocaleCategory::LcAll, "");
    }
    gettextrs::bindtextdomain(GETTEXT_PACKAGE, LOCALEDIR).expect("Unable to bind the text domain");
    gettextrs::bind_textdomain_codeset(GETTEXT_PACKAGE, "UTF-8")
        .expect("Unable to set the text domain encoding");
    gettextrs::textdomain(GETTEXT_PACKAGE).expect("Unable to switch to the text domain");

    gio::resources_register_include!("resources.gresource").expect("Could not load resources");

    glib::set_application_name(&gettext("Mirage"));

    MirageApplication::new().run()
}
