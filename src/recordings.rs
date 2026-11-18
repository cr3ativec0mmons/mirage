//! Location and naming of screen recordings.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use gtk::glib;

use crate::config::RECORDINGS_FOLDER;

/// The `Mirage` folder inside the Videos directory of the user.
pub fn recordings_dir() -> PathBuf {
    glib::user_special_dir(glib::UserDirectory::Videos)
        .unwrap_or_else(|| glib::home_dir().join("Videos"))
        .join(RECORDINGS_FOLDER)
}

/// Creates the recordings folder if needed and returns an unused file path.
pub fn new_recording_path(device_name: &str, extension: &str) -> io::Result<PathBuf> {
    let dir = recordings_dir();
    fs::create_dir_all(&dir)?;

    let timestamp = glib::DateTime::now_local()
        .and_then(|now| now.format("%Y-%m-%d %H-%M-%S"))
        .map(|timestamp| timestamp.to_string())
        .unwrap_or_default();
    let base = format!("{} {timestamp}", file_name_safe(device_name));
    Ok(unique_path(&dir, base.trim(), extension))
}

fn unique_path(dir: &Path, base: &str, extension: &str) -> PathBuf {
    let mut path = dir.join(format!("{base}.{extension}"));
    let mut counter = 2;
    while path.exists() {
        path = dir.join(format!("{base} ({counter}).{extension}"));
        counter += 1;
    }
    path
}

fn file_name_safe(name: &str) -> String {
    let name: String = name
        .chars()
        .map(|c| if c == '/' || c.is_control() { '_' } else { c })
        .collect();
    let name = name.trim().trim_start_matches('.');
    if name.is_empty() {
        "Android".to_owned()
    } else {
        name.to_owned()
    }
}

/// A path for display, with the home directory shortened to `~`.
pub fn display_path(path: &Path) -> String {
    match path.strip_prefix(glib::home_dir()) {
        Ok(relative) => Path::new("~").join(relative).display().to_string(),
        Err(_) => path.display().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn makes_names_safe_for_files() {
        assert_eq!(file_name_safe("Pixel 8 Pro"), "Pixel 8 Pro");
        assert_eq!(file_name_safe("a/b\nc"), "a_b_c");
        assert_eq!(file_name_safe(" ..hidden "), "hidden");
        assert_eq!(file_name_safe("   "), "Android");
    }

    #[test]
    fn avoids_existing_files() {
        let dir = std::env::temp_dir().join(format!("mirage-test-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let first = unique_path(&dir, "Phone", "mp4");
        assert_eq!(first, dir.join("Phone.mp4"));
        fs::write(&first, b"").unwrap();
        assert_eq!(unique_path(&dir, "Phone", "mp4"), dir.join("Phone (2).mp4"));
        fs::remove_dir_all(&dir).unwrap();
    }
}
