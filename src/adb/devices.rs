use std::collections::HashMap;

/// Keys that `adb devices -l` appends after the state of each device.
const PROPERTY_KEYS: &[&str] = &["usb", "product", "model", "device", "transport_id"];

/// A device as reported by the ADB server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceInfo {
    pub serial: String,
    /// The raw state, like `device`, `unauthorized` or `no permissions (...)`.
    pub state: String,
    pub properties: HashMap<String, String>,
}

impl DeviceInfo {
    pub fn property(&self, key: &str) -> Option<&str> {
        self.properties.get(key).map(String::as_str)
    }

    /// The model name with underscores replaced, like `Pixel 8 Pro`.
    pub fn model(&self) -> Option<String> {
        self.property("model").map(|model| model.replace('_', " "))
    }

    /// Whether the device is connected over the network rather than USB.
    pub fn is_wireless(&self) -> bool {
        self.property("usb").is_none()
            && (self.serial.contains("._adb-tls-connect.") || self.serial.contains(':'))
    }
}

/// Parses the long format of the device list (`adb devices -l`).
pub fn parse_device_list(text: &str) -> Vec<DeviceInfo> {
    text.lines().filter_map(parse_line).collect()
}

fn parse_line(line: &str) -> Option<DeviceInfo> {
    let mut tokens = line.split_whitespace();
    let serial = tokens.next()?.to_owned();

    let mut state = Vec::new();
    let mut properties = HashMap::new();
    for token in tokens {
        match token.split_once(':') {
            Some((key, value)) if PROPERTY_KEYS.contains(&key) => {
                properties.insert(key.to_owned(), value.to_owned());
            }
            // Multi-word states such as `no permissions (...)` may contain URLs
            _ if properties.is_empty() => state.push(token),
            _ => {}
        }
    }

    if state.is_empty() {
        return None;
    }

    Some(DeviceInfo {
        serial,
        state: state.join(" "),
        properties,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_usb_device() {
        let devices = parse_device_list(
            "0A121FDD4003XQ         device usb:1-4 product:husky model:Pixel_8_Pro device:husky transport_id:3\n",
        );
        assert_eq!(devices.len(), 1);
        let device = &devices[0];
        assert_eq!(device.serial, "0A121FDD4003XQ");
        assert_eq!(device.state, "device");
        assert_eq!(device.model().as_deref(), Some("Pixel 8 Pro"));
        assert_eq!(device.property("transport_id"), Some("3"));
        assert!(!device.is_wireless());
    }

    #[test]
    fn parses_wireless_devices() {
        let devices = parse_device_list(concat!(
            "192.168.1.20:37099     device product:husky model:Pixel_8_Pro device:husky transport_id:4\n",
            "adb-0A121FDD4003XQ-Ab1Cd2._adb-tls-connect._tcp device product:a model:b device:c transport_id:5\n",
        ));
        assert_eq!(devices.len(), 2);
        assert!(devices.iter().all(DeviceInfo::is_wireless));
    }

    #[test]
    fn parses_multi_word_states() {
        let devices = parse_device_list(
            "0A121FDD4003XQ  no permissions (missing udev rules? user is in the plugdev group); see [http://developer.android.com/tools/device.html] usb:1-4 transport_id:1\n",
        );
        assert_eq!(devices.len(), 1);
        assert!(devices[0].state.starts_with("no permissions"));
        assert!(devices[0].state.ends_with("device.html]"));
        assert_eq!(devices[0].property("usb"), Some("1-4"));
        assert_eq!(devices[0].model(), None);
    }

    #[test]
    fn parses_unauthorized_device() {
        let devices = parse_device_list("0A121FDD4003XQ unauthorized usb:1-4 transport_id:2");
        assert_eq!(devices[0].state, "unauthorized");
    }

    #[test]
    fn ignores_empty_lines() {
        assert!(parse_device_list("\n\n").is_empty());
        assert!(parse_device_list("").is_empty());
    }
}
