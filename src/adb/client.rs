//! A minimal client for the ADB server protocol.
//!
//! Requests are sent as a four digit hexadecimal length followed by the
//! service name, and the server answers `OKAY` or `FAIL` followed by a
//! length-prefixed message. See `SERVICES.TXT` in the ADB sources.

use std::io::{self, Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::process::Command;
use std::time::Duration;
use std::{env, fmt};

use super::devices::{DeviceInfo, parse_device_list};

const DEFAULT_PORT: u16 = 5037;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const QUERY_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug)]
pub enum Error {
    /// The server could not be reached.
    Io(io::Error),
    /// The server refused the request, with its explanation.
    Server(String),
    /// The server sent something unexpected.
    Protocol(String),
}

impl Error {
    /// Whether the server is not running, so starting it may help.
    pub fn is_server_down(&self) -> bool {
        matches!(self, Self::Io(error) if matches!(
            error.kind(),
            io::ErrorKind::ConnectionRefused | io::ErrorKind::NotFound | io::ErrorKind::TimedOut
        ))
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "{error}"),
            Self::Server(message) => f.write_str(message),
            Self::Protocol(message) => write!(f, "Unexpected reply from the ADB server: {message}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<io::Error> for Error {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

pub type Result<T> = std::result::Result<T, Error>;

/// Connection details of the ADB server, which every request opens anew.
#[derive(Debug, Clone)]
pub struct AdbClient {
    host: String,
    port: u16,
}

impl Default for AdbClient {
    fn default() -> Self {
        Self::from_env()
    }
}

impl AdbClient {
    /// Uses the same environment variables as the `adb` command.
    pub fn from_env() -> Self {
        if let Some((host, port)) = env::var("ADB_SERVER_SOCKET")
            .ok()
            .as_deref()
            .and_then(|socket| socket.strip_prefix("tcp:"))
            .and_then(|address| address.rsplit_once(':'))
            .and_then(|(host, port)| Some((host.to_owned(), port.parse().ok()?)))
        {
            return Self { host, port };
        }

        let host =
            env::var("ANDROID_ADB_SERVER_ADDRESS").unwrap_or_else(|_| "localhost".to_owned());
        let port = env::var("ANDROID_ADB_SERVER_PORT")
            .ok()
            .and_then(|port| port.parse().ok())
            .unwrap_or(DEFAULT_PORT);
        Self { host, port }
    }

    fn open(&self, service: &str) -> Result<TcpStream> {
        let mut last_error = io::Error::from(io::ErrorKind::NotFound);
        for address in (self.host.as_str(), self.port).to_socket_addrs()? {
            match TcpStream::connect_timeout(&address, CONNECT_TIMEOUT) {
                Ok(mut stream) => {
                    stream.set_read_timeout(Some(QUERY_TIMEOUT))?;
                    stream.set_write_timeout(Some(QUERY_TIMEOUT))?;
                    send_request(&mut stream, service)?;
                    return Ok(stream);
                }
                Err(error) => last_error = error,
            }
        }
        Err(last_error.into())
    }

    /// Sends a host request and returns its reply.
    pub fn query(&self, service: &str) -> Result<String> {
        let mut stream = self.open(service)?;
        read_string(&mut stream)
    }

    /// Connects to a device over the network.
    pub fn connect_device(&self, address: &str) -> Result<String> {
        let reply = self.query(&format!("host:connect:{address}"))?;
        if reply.starts_with("connected to") || reply.starts_with("already connected to") {
            Ok(reply)
        } else {
            Err(Error::Server(reply))
        }
    }

    /// Pairs with a device that is showing a pairing code or a QR code.
    pub fn pair(&self, address: &str, code: &str) -> Result<String> {
        let reply = self.query(&format!("host:pair:{code}:{address}"))?;
        if reply.starts_with("Successfully paired") {
            Ok(reply)
        } else {
            Err(Error::Server(
                reply.strip_prefix("Failed: ").unwrap_or(&reply).to_owned(),
            ))
        }
    }

    /// Runs a shell command on a device and returns its output.
    pub fn shell(&self, serial: &str, command: &str) -> Result<String> {
        let mut stream = self.open(&format!("host:transport:{serial}"))?;
        send_request(&mut stream, &format!("shell:{command}"))?;
        let mut output = String::new();
        stream.read_to_string(&mut output)?;
        Ok(output)
    }

    /// Opens a stream that sends the device list every time it changes.
    pub fn track_devices(&self) -> Result<DeviceListStream> {
        let stream = self.open("host:track-devices-l")?;
        stream.set_read_timeout(None)?;
        Ok(DeviceListStream(stream))
    }
}

pub struct DeviceListStream(TcpStream);

impl DeviceListStream {
    /// Blocks until the device list changes.
    pub fn next_list(&mut self) -> Result<Vec<DeviceInfo>> {
        read_string(&mut self.0).map(|text| parse_device_list(&text))
    }
}

fn send_request(stream: &mut (impl Read + Write), service: &str) -> Result<()> {
    if service.len() > 0xffff {
        return Err(Error::Protocol("request too long".to_owned()));
    }
    write!(stream, "{:04x}{service}", service.len())?;
    stream.flush()?;

    let mut status = [0; 4];
    stream.read_exact(&mut status)?;
    match &status {
        b"OKAY" => Ok(()),
        b"FAIL" => Err(Error::Server(read_string(stream)?)),
        other => Err(Error::Protocol(String::from_utf8_lossy(other).into_owned())),
    }
}

fn read_string(stream: &mut impl Read) -> Result<String> {
    let mut length = [0; 4];
    stream.read_exact(&mut length)?;
    let length = std::str::from_utf8(&length)
        .ok()
        .and_then(|length| usize::from_str_radix(length, 16).ok())
        .ok_or_else(|| Error::Protocol(String::from_utf8_lossy(&length).into_owned()))?;

    let mut data = vec![0; length];
    stream.read_exact(&mut data)?;
    Ok(String::from_utf8_lossy(&data).into_owned())
}

/// The `adb` executable, which can be overridden with the `ADB` variable like
/// scrcpy does.
pub fn adb_program() -> String {
    env::var("ADB").unwrap_or_else(|_| "adb".to_owned())
}

/// Starts the ADB server in the background, if it is not running yet.
pub fn start_server() -> std::result::Result<(), String> {
    let output = Command::new(adb_program())
        .arg("start-server")
        .output()
        .map_err(|error| format!("{}: {error}", adb_program()))?;
    if output.status.success() {
        Ok(())
    } else {
        let message = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        Err(if message.is_empty() {
            output.status.to_string()
        } else {
            message
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    /// An in-memory stream with canned replies that records what was written.
    struct FakeStream {
        input: Cursor<Vec<u8>>,
        output: Vec<u8>,
    }

    impl FakeStream {
        fn new(input: &[u8]) -> Self {
            Self {
                input: Cursor::new(input.to_vec()),
                output: Vec::new(),
            }
        }
    }

    impl Read for FakeStream {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            self.input.read(buf)
        }
    }

    impl Write for FakeStream {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.output.write(buf)
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn sends_length_prefixed_requests() {
        let mut stream = FakeStream::new(b"OKAY");
        send_request(&mut stream, "host:version").unwrap();
        assert_eq!(stream.output, b"000chost:version");
    }

    #[test]
    fn reports_server_failures() {
        let mut stream = FakeStream::new(b"FAIL0010device not found");
        match send_request(&mut stream, "host:transport:x") {
            Err(Error::Server(message)) => assert_eq!(message, "device not found"),
            other => panic!("unexpected result: {other:?}"),
        }
    }

    #[test]
    fn rejects_unknown_status() {
        let mut stream = FakeStream::new(b"WHAT");
        assert!(matches!(
            send_request(&mut stream, "host:version"),
            Err(Error::Protocol(_))
        ));
    }

    #[test]
    fn reads_length_prefixed_strings() {
        let payload = "emulator-5554\tdevice transport_id:1\n";
        let mut stream = Cursor::new(format!("{:04x}{payload}", payload.len()).into_bytes());
        let text = read_string(&mut stream).unwrap();
        let devices = parse_device_list(&text);
        assert_eq!(devices[0].serial, "emulator-5554");
    }

    #[test]
    fn reads_empty_device_list() {
        let mut stream = Cursor::new(b"0000".to_vec());
        assert_eq!(read_string(&mut stream).unwrap(), "");
    }
}
