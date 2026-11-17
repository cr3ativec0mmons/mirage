//! Build-time configuration, filled in by Meson through environment variables.
//!
//! Plain `cargo` builds fall back to development defaults, which is enough for
//! `cargo check`, `cargo clippy` and `cargo test`.

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
pub const PKGDATADIR: &str = env_or!("MIRAGE_PKGDATADIR", "/usr/share/mirage");

/// The resource path is the same for every profile, unlike the application ID.
pub const RESOURCE_BASE_PATH: &str = "/tech/vitainside/mirage";

/// Name of the folder inside the Videos directory where recordings are saved.
/// It must match the `xdg-videos/Mirage` permission of the Flatpak manifest.
pub const RECORDINGS_FOLDER: &str = "Mirage";

pub fn is_devel() -> bool {
    PROFILE == "Devel"
}
