//! Follows the device list of the ADB server from a background thread.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
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
    /// The tracker started the ADB server, which has no wireless devices yet.
    ServerStarted,
    /// The ADB server could not be started or reached.
    Unavailable(String),
}

/// Keeps the tracker running. It stops when the handle and the receiver are dropped.
pub struct TrackerHandle {
    client: AdbClient,
    retry: mpsc::Sender<()>,
    started_server: Arc<AtomicBool>,
    stopped: Arc<AtomicBool>,
}

impl TrackerHandle {
    /// Retries right away after the server was unavailable.
    pub fn retry(&self) {
        let _ = self.retry.send(());
    }

    /// Whether the tracker started the ADB server, so Mirage may restart it.
    pub fn started_server(&self) -> bool {
        self.started_server.load(Ordering::SeqCst)
    }

    /// Stops following the device list, and stops the ADB server if the
    /// tracker started it, so it does not outlive Mirage. A server that was
    /// already running is left alone, as other programs may be using it.
    pub fn shutdown(&self) {
        self.stopped.store(true, Ordering::SeqCst);
        if self.started_server.load(Ordering::SeqCst) {
            tracing::info!("Stopping the ADB server");
            if let Err(error) = self.client.kill_server() {
                tracing::debug!("Could not stop the ADB server: {error}");
            }
        }
    }
}

pub fn spawn_tracker(client: AdbClient) -> (TrackerHandle, async_channel::Receiver<TrackerEvent>) {
    let (sender, receiver) = async_channel::unbounded();
    let (retry_sender, retry_receiver) = mpsc::channel();
    let started_server = Arc::new(AtomicBool::new(false));
    let stopped = Arc::new(AtomicBool::new(false));
    let handle = TrackerHandle {
        client: client.clone(),
        retry: retry_sender,
        started_server: started_server.clone(),
        stopped: stopped.clone(),
    };

    thread::Builder::new()
        .name("adb-tracker".to_owned())
        .spawn(move || {
            loop {
                let delay = match track(&client, &sender, &started_server) {
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
                if sender.is_closed() || stopped.load(Ordering::SeqCst) {
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

    (handle, receiver)
}

/// Follows the device list until the connection to the server is lost.
fn track(
    client: &AdbClient,
    sender: &async_channel::Sender<TrackerEvent>,
    started_server: &AtomicBool,
) -> Result<(), String> {
    let mut stream = match client.track_devices() {
        Ok(stream) => stream,
        Err(error) if error.is_server_down() => {
            tracing::info!("Starting the ADB server");
            start_server()?;
            started_server.store(true, Ordering::SeqCst);
            let _ = sender.send_blocking(TrackerEvent::ServerStarted);
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
