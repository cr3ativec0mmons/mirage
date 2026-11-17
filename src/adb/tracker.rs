//! Follows the device list of the ADB server from a background thread.

use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use super::client::{AdbClient, start_server};
use super::devices::DeviceInfo;

const RETRY_DELAY: Duration = Duration::from_secs(3);
const RECONNECT_DELAY: Duration = Duration::from_millis(500);

#[derive(Debug)]
pub enum TrackerEvent {
    /// The complete list of devices, sent every time it changes.
    Devices(Vec<DeviceInfo>),
    /// The ADB server could not be started or reached.
    Unavailable(String),
}

/// Keeps the tracker running. It stops when the handle and the receiver are dropped.
pub struct TrackerHandle {
    retry: mpsc::Sender<()>,
}

impl TrackerHandle {
    /// Retries right away after the server was unavailable.
    pub fn retry(&self) {
        let _ = self.retry.send(());
    }
}

pub fn spawn_tracker(client: AdbClient) -> (TrackerHandle, async_channel::Receiver<TrackerEvent>) {
    let (sender, receiver) = async_channel::unbounded();
    let (retry_sender, retry_receiver) = mpsc::channel();

    thread::Builder::new()
        .name("adb-tracker".to_owned())
        .spawn(move || {
            loop {
                let delay = match track(&client, &sender) {
                    Ok(()) => RECONNECT_DELAY,
                    Err(message) => {
                        tracing::warn!("ADB server unavailable: {message}");
                        if sender
                            .send_blocking(TrackerEvent::Unavailable(message))
                            .is_err()
                        {
                            return;
                        }
                        RETRY_DELAY
                    }
                };
                if sender.is_closed() {
                    return;
                }
                if let Err(mpsc::RecvTimeoutError::Disconnected) =
                    retry_receiver.recv_timeout(delay)
                {
                    return;
                }
            }
        })
        .expect("Could not start the device tracker thread");

    (
        TrackerHandle {
            retry: retry_sender,
        },
        receiver,
    )
}

/// Follows the device list until the connection to the server is lost.
fn track(client: &AdbClient, sender: &async_channel::Sender<TrackerEvent>) -> Result<(), String> {
    let mut stream = match client.track_devices() {
        Ok(stream) => stream,
        Err(error) if error.is_server_down() => {
            tracing::info!("Starting the ADB server");
            start_server()?;
            client.track_devices().map_err(|error| error.to_string())?
        }
        Err(error) => return Err(error.to_string()),
    };

    loop {
        match stream.next_list() {
            Ok(devices) => {
                if sender
                    .send_blocking(TrackerEvent::Devices(devices))
                    .is_err()
                {
                    return Ok(());
                }
            }
            Err(error) => {
                tracing::info!("Lost the connection to the ADB server: {error}");
                return Ok(());
            }
        }
    }
}
