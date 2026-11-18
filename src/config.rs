//! Build-time configuration, filled in by Meson through environment variables.
//!
//! Plain `cargo` builds fall back to development defaults, which is enough for
//! `cargo run`, with the UI resources and settings schema built by `build.rs`.

use gtk::gio;

macro_rules! env_or {
    ($name:literal, $default:expr) => {
        match option_env!($name) {
            Some(value) => value,
            None => $default,
        }
    };
}

pub const APP_ID: &str = env_or!("MIRAGE_APP_ID", "tech.vitainside.mirage.Devel");
pub const PROFILE: &str = env_or!("MIRAGE_PROFILE", "Devel");
pub const VERSION: &str = env_or!("MIRAGE_VERSION", env!("CARGO_PKG_VERSION"));
pub const GETTEXT_PACKAGE: &str = env_or!("MIRAGE_GETTEXT_PACKAGE", "mirage");
pub const LOCALEDIR: &str = env_or!("MIRAGE_LOCALEDIR", "/usr/share/locale");

/// The resource path is the same for every profile, unlike the application ID.
pub const RESOURCE_BASE_PATH: &str = "/tech/vitainside/mirage";

/// Name of the folder inside the Videos directory where recordings are saved.
/// It must match the `xdg-videos/Mirage` permission of the Flatpak manifest.
pub const RECORDINGS_FOLDER: &str = "Mirage";

pub fn is_devel() -> bool {
    PROFILE == "Devel"
}

/// The settings of the app, from the schema compiled by `build.rs` in plain
/// `cargo` builds, which have no installed schema.
pub fn settings() -> gio::Settings {
    let Some(dir) = option_env!("MIRAGE_SCHEMA_DIR") else {
        return gio::Settings::new(APP_ID);
    };
    let default = gio::SettingsSchemaSource::default();
    let schema = gio::SettingsSchemaSource::from_directory(dir, default.as_ref(), false)
        .ok()
        .and_then(|source| source.lookup(APP_ID, false))
        .expect("Could not load the settings schema");
    gio::Settings::new_full(&schema, None::<&gio::SettingsBackend>, None)
}
