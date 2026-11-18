use std::cell::Cell;
use std::ffi::{OsStr, OsString};
use std::rc::Rc;

use gtk::{gio, glib};

use crate::config::APP_ID;

const SIGTERM: i32 = 15;

/// How a scrcpy process ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// The window was closed, or the session was stopped from Mirage.
    Finished,
    /// The device was disconnected while mirroring.
    Disconnected,
    /// scrcpy failed, with the error messages it printed.
    Failed(String),
}

/// A running scrcpy process.
pub struct Session {
    process: gio::Subprocess,
    stopped: Rc<Cell<bool>>,
}

impl Session {
    /// Starts scrcpy, and calls `on_exit` on the main loop once it ends.
    pub fn launch(
        args: &[OsString],
        on_exit: impl FnOnce(Outcome) + 'static,
    ) -> Result<Self, glib::Error> {
        let launcher = gio::SubprocessLauncher::new(
            gio::SubprocessFlags::STDOUT_PIPE | gio::SubprocessFlags::STDERR_MERGE,
        );
        // Groups the scrcpy windows with Mirage in the shell
        launcher.setenv("SDL_APP_ID", APP_ID, true);

        let program = std::env::var_os("MIRAGE_SCRCPY").unwrap_or_else(|| "scrcpy".into());
        let argv: Vec<&OsStr> = std::iter::once(program.as_os_str())
            .chain(args.iter().map(OsString::as_os_str))
            .collect();
        tracing::info!("Running {argv:?}");
        let process = launcher.spawn(&argv)?;

        let stopped = Rc::new(Cell::new(false));
        glib::spawn_future_local({
            let process = process.clone();
            let stopped = stopped.clone();
            async move {
                // Collects the output until scrcpy exits
                let log = match process.communicate_future(None).await {
                    Ok((stdout, _)) => stdout
                        .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
                        .unwrap_or_default(),
                    Err(error) => {
                        tracing::warn!("Could not read the output of scrcpy: {error}");
                        String::new()
                    }
                };
                tracing::debug!("scrcpy output:\n{log}");

                let outcome = if stopped.get() {
                    Outcome::Finished
                } else if process.has_exited() {
                    match process.exit_status() {
                        0 => Outcome::Finished,
                        2 => Outcome::Disconnected,
                        _ => Outcome::Failed(error_message(&log)),
                    }
                } else {
                    Outcome::Failed(error_message(&log))
                };
                on_exit(outcome);
            }
        });

        Ok(Self { process, stopped })
    }

    /// Asks scrcpy to quit, which also finishes writing the recording.
    pub fn stop(&self) {
        self.stopped.set(true);
        self.process.send_signal(SIGTERM);
    }

    /// Kills scrcpy, for when it does not respond to [`Self::stop`].
    pub fn kill(&self) {
        self.stopped.set(true);
        self.process.force_exit();
    }
}

/// Extracts the error messages from the output of scrcpy.
fn error_message(log: &str) -> String {
    let errors: Vec<&str> = log
        .lines()
        .filter_map(|line| line.strip_prefix("ERROR: "))
        .map(str::trim)
        .collect();
    if errors.is_empty() {
        log.lines()
            .rev()
            .find(|line| !line.trim().is_empty())
            .unwrap_or_default()
            .trim()
            .to_owned()
    } else {
        errors.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_error_lines() {
        let log = "scrcpy 5.0.1 <https://github.com/Genymobile/scrcpy>\n\
                   INFO: ADB device found:\n\
                   ERROR: Could not find any ADB device\n\
                   ERROR: Server connection failed\n";
        assert_eq!(
            error_message(log),
            "Could not find any ADB device\nServer connection failed"
        );
    }

    #[test]
    fn falls_back_to_last_line() {
        assert_eq!(
            error_message("first\nsomething broke\n\n"),
            "something broke"
        );
        assert_eq!(error_message(""), "");
    }
}
