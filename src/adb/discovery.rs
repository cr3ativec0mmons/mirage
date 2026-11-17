//! Discovery of devices with wireless debugging turned on, using mDNS.
//!
//! Android advertises `_adb-tls-pairing._tcp` while it shows a pairing code or
//! waits for a QR code to be scanned, and `_adb-tls-connect._tcp` while
//! wireless debugging is on.

use std::net::IpAddr;

use gtk::glib;
use mdns_sd::{ResolvedService, ServiceDaemon, ServiceEvent};

pub const PAIRING_SERVICE: &str = "_adb-tls-pairing._tcp.local.";
pub const CONNECT_SERVICE: &str = "_adb-tls-connect._tcp.local.";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Service {
    /// The instance name, like `adb-0A121FDD4003XQ-Ab1Cd2`.
    pub instance: String,
    pub fullname: String,
    pub ip: IpAddr,
    /// The address in the form expected by ADB, `host:port`.
    pub address: String,
}

#[derive(Debug, Clone)]
pub enum Event {
    Resolved(Service),
    Removed { fullname: String },
}

pub struct Discovery {
    daemon: ServiceDaemon,
}

impl Discovery {
    pub fn new() -> Result<Self, mdns_sd::Error> {
        Ok(Self {
            daemon: ServiceDaemon::new()?,
        })
    }

    /// Calls `callback` on the main loop for every change of the services of
    /// the given type, until [`Self::stop_browse`] is called.
    pub fn browse(
        &self,
        service_type: &str,
        callback: impl Fn(Event) + 'static,
    ) -> Result<(), mdns_sd::Error> {
        let receiver = self.daemon.browse(service_type)?;
        glib::spawn_future_local(async move {
            while let Ok(event) = receiver.recv_async().await {
                match event {
                    ServiceEvent::ServiceResolved(info) => {
                        if let Some(service) = service_from_info(&info) {
                            callback(Event::Resolved(service));
                        }
                    }
                    ServiceEvent::ServiceRemoved(_, fullname) => {
                        callback(Event::Removed { fullname });
                    }
                    ServiceEvent::SearchStopped(_) => break,
                    _ => {}
                }
            }
        });
        Ok(())
    }

    pub fn stop_browse(&self, service_type: &str) {
        if let Err(error) = self.daemon.stop_browse(service_type) {
            tracing::debug!("Could not stop browsing {service_type}: {error}");
        }
    }
}

impl Drop for Discovery {
    fn drop(&mut self) {
        let _ = self.daemon.shutdown();
    }
}

fn service_from_info(info: &ResolvedService) -> Option<Service> {
    let ip = preferred_address(info.addresses.iter().map(|address| address.to_ip_addr()))?;
    Some(Service {
        instance: mdns_sd::instance_name_unescaped(&info.fullname),
        fullname: info.fullname.clone(),
        ip,
        address: format_address(ip, info.port),
    })
}

/// Prefers IPv4, then IPv6 addresses that do not need a scope.
fn preferred_address(addresses: impl Iterator<Item = IpAddr>) -> Option<IpAddr> {
    addresses
        .filter(|ip| match ip {
            IpAddr::V4(_) => true,
            IpAddr::V6(ip) => !ip.is_unicast_link_local(),
        })
        .min_by_key(|ip| (ip.is_ipv6(), *ip))
}

fn format_address(ip: IpAddr, port: u16) -> String {
    match ip {
        IpAddr::V4(ip) => format!("{ip}:{port}"),
        IpAddr::V6(ip) => format!("[{ip}]:{port}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefers_ipv4_addresses() {
        let addresses: Vec<IpAddr> = vec![
            "fe80::1".parse().unwrap(),
            "2001:db8::5".parse().unwrap(),
            "192.168.1.20".parse().unwrap(),
        ];
        assert_eq!(
            preferred_address(addresses.into_iter()),
            Some("192.168.1.20".parse().unwrap())
        );
    }

    #[test]
    fn skips_link_local_ipv6_addresses() {
        let addresses: Vec<IpAddr> = vec!["fe80::1".parse().unwrap()];
        assert_eq!(preferred_address(addresses.into_iter()), None);

        let addresses: Vec<IpAddr> =
            vec!["fe80::1".parse().unwrap(), "2001:db8::5".parse().unwrap()];
        assert_eq!(
            preferred_address(addresses.into_iter()),
            Some("2001:db8::5".parse().unwrap())
        );
    }

    #[test]
    fn formats_addresses_for_adb() {
        assert_eq!(
            format_address("192.168.1.20".parse().unwrap(), 37099),
            "192.168.1.20:37099"
        );
        assert_eq!(
            format_address("2001:db8::5".parse().unwrap(), 5555),
            "[2001:db8::5]:5555"
        );
    }
}
