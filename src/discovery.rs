use std::net::{IpAddr, SocketAddr};
use std::sync::OnceLock;

use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};

use crate::config::ServerIcon;
use crate::icons;

const SERVICE_TYPE: &str = "_spud._tcp.local.";

/// Return all usable interface addresses in order:
/// IPv4 ascending, then IPv6 ascending.
pub fn list_interface_addrs() -> Vec<IpAddr> {
    let mut addrs: Vec<IpAddr> = if_addrs::get_if_addrs()
        .unwrap_or_default()
        .into_iter()
        .filter(|iface| !iface.is_loopback())
        .map(|iface| iface.ip())
        .filter(|ip| !ip.is_multicast() && !ip.is_unspecified())
        .filter(|ip| match ip {
            IpAddr::V4(a) => !a.is_link_local(),
            IpAddr::V6(a) => !a.is_unicast_link_local(),
        })
        .collect();

    addrs.sort();
    addrs.dedup();

    // Move IPv4 before IPv6
    addrs.sort_by(|a, b| match (a.is_ipv4(), b.is_ipv4()) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.cmp(b),
    });

    addrs
}

/// Build the list of options for the server bind-address dropdown.
pub fn bind_options() -> Vec<String> {
    let mut opts = vec!["0.0.0.0".to_string()];
    for addr in list_interface_addrs() {
        opts.push(addr.to_string());
    }
    opts
}

/// Return a human-friendly endpoint string for the status page.
pub fn display_endpoint(bind_address: &str, port: u16) -> String {
    if bind_address.is_empty() {
        format!("0.0.0.0:{port}")
    } else {
        format!("{bind_address}:{port}")
    }
}

#[derive(Debug, Clone)]
pub struct DiscoveredServer {
    pub fullname: String,
    pub name: String,
    pub host: String,
    pub port: String,
    pub address: String,
    pub addrs: Vec<SocketAddr>,
    pub icon: char,
    pub auth: bool,
    pub encrypt: bool,
}

#[derive(Debug, Clone)]
pub enum Event {
    Found(DiscoveredServer),
    Lost(String),
}

fn daemon() -> Option<&'static ServiceDaemon> {
    static DAEMON: OnceLock<Option<ServiceDaemon>> = OnceLock::new();
    DAEMON
        .get_or_init(|| {
            ServiceDaemon::new()
                .map_err(|e| eprintln!("[spud] mDNS daemon: {e}"))
                .ok()
        })
        .as_ref()
}

pub fn browse() -> impl iced::futures::Stream<Item = Event> + Send + 'static {
    iced::stream::channel(64, |mut tx: iced::futures::channel::mpsc::Sender<Event>| async move {
        let d = match daemon() {
            Some(d) => d,
            None => {
                std::future::pending::<()>().await;
                return;
            }
        };
        let receiver = match d.browse(SERVICE_TYPE) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("[spud] mDNS browse: {e}");
                std::future::pending::<()>().await;
                return;
            }
        };
        loop {
            match receiver.recv_async().await {
                Ok(ServiceEvent::SearchStarted(_)) => {}
                Ok(ServiceEvent::ServiceFound(_, _)) => {}
                Ok(ServiceEvent::ServiceResolved(info)) => {
                    let fullname = info.get_fullname().to_string();
                    let name = info
                        .get_properties()
                        .get("name")
                        .map(|r| r.val_str().to_string())
                        .unwrap_or_else(|| strip_service_type(&fullname));
                    let host = info
                        .get_properties()
                        .get("hostname")
                        .map(|r| r.val_str().trim_end_matches('.').to_string())
                        .unwrap_or_else(|| info.get_hostname().trim_end_matches('.').to_string());
                    let port = info.get_port().to_string();
                    let icon = resolve_icon(&info);
                    let auth = info
                        .get_properties()
                        .get("auth")
                        .map(|r| r.val_str() == "true")
                        .unwrap_or(false);
                    let encrypt = info
                        .get_properties()
                        .get("encrypt")
                        .map(|r| r.val_str() == "true")
                        .unwrap_or(false);
                    let address = format!("{}:{}", host, port);
                    let mut addrs: Vec<SocketAddr> = info
                        .get_addresses()
                        .iter()
                        .filter(|ip| {
                            !ip.is_loopback()
                                && !ip.is_multicast()
                                && !ip.is_unspecified()
                                && !match ip {
                                    std::net::IpAddr::V4(a) => a.is_link_local(),
                                    std::net::IpAddr::V6(a) => a.is_unicast_link_local(),
                                }
                        })
                        .map(|ip| SocketAddr::new(*ip, info.get_port()))
                        .collect();
                    // Deterministic ordering: IPv4 before IPv6
                    addrs.sort_by(|a, b| {
                        match (a.is_ipv4(), b.is_ipv4()) {
                            (true, false) => std::cmp::Ordering::Less,
                            (false, true) => std::cmp::Ordering::Greater,
                            _ => a.ip().cmp(&b.ip()),
                        }
                    });
                    let _ = tx.try_send(Event::Found(DiscoveredServer {
                        fullname,
                        name,
                        host,
                        port,
                        address,
                        addrs,
                        icon,
                        auth,
                        encrypt,
                    }));
                }
                Ok(ServiceEvent::ServiceRemoved(_, ref fullname)) => {
                    let _ = tx.try_send(Event::Lost(fullname.clone()));
                }
                Ok(_) => {}
                Err(e) => {
                    eprintln!("[spud] mDNS browse error: {e:?}");
                    std::future::pending::<()>().await;
                    return;
                }
            }
        }
    })
}

pub struct Registration {
    fullname: String,
}

impl Registration {
    pub fn new(name: &str, port: u16, bind_address: &str, icon: ServerIcon, require_auth: bool, encrypt_udp: bool) -> Option<Self> {
        let d = daemon()?;
        let hostname = hostname::get()
            .ok()
            .and_then(|h| h.into_string().ok())
            .unwrap_or_else(|| "spud".to_string());
        let fqdn = format!("{}.local.", hostname);
        let icon_str = match icon {
            ServerIcon::Desktop => "desktop",
            ServerIcon::Laptop => "laptop",
            ServerIcon::Server => "server",
        };
        let mut props = std::collections::HashMap::new();
        props.insert("icon".to_string(), icon_str.to_string());
        props.insert("name".to_string(), name.to_string());
        props.insert("auth".to_string(), require_auth.to_string());
        props.insert("encrypt".to_string(), encrypt_udp.to_string());
        props.insert("hostname".to_string(), fqdn.clone());
        let instance_name = format!("{}-{}-{}", name, port, std::process::id());
        // Use a unique hostname per instance so A/AAAA records don't collide
        // across machines that share the same system hostname.
        let fqdn = format!("{}.local.", instance_name);

        // Determine which IP address(es) to advertise.
        let info = if bind_address == "0.0.0.0" || bind_address == "::" || bind_address.is_empty() {
            eprintln!("[spud] mDNS: advertising {instance_name} on all interfaces");
            ServiceInfo::new(SERVICE_TYPE, &instance_name, &fqdn, (), port, Some(props))
                .map_err(|e| eprintln!("[spud] mDNS ServiceInfo: {e}"))
                .ok()?
                .enable_addr_auto()
        } else {
            match bind_address.parse::<IpAddr>() {
                Ok(ip) => {
                    eprintln!("[spud] mDNS: advertising {instance_name} on [{ip}]");
                    ServiceInfo::new(SERVICE_TYPE, &instance_name, &fqdn, &[ip][..], port, Some(props))
                        .map_err(|e| eprintln!("[spud] mDNS ServiceInfo: {e}"))
                        .ok()?
                }
                Err(_) => {
                    eprintln!("[spud] mDNS: invalid bind_address '{bind_address}', advertising on all interfaces");
                    ServiceInfo::new(SERVICE_TYPE, &instance_name, &fqdn, (), port, Some(props))
                        .map_err(|e| eprintln!("[spud] mDNS ServiceInfo: {e}"))
                        .ok()?
                        .enable_addr_auto()
                }
            }
        };

        let fullname = info.get_fullname().to_string();
        d.register(info)
            .map_err(|e| eprintln!("[spud] mDNS register: {e}"))
            .ok()?;
        Some(Self { fullname })
    }
}

impl Registration {
    pub fn fullname(&self) -> &str {
        &self.fullname
    }
}

impl Drop for Registration {
    fn drop(&mut self) {
        if let Some(d) = daemon() {
            if let Ok(receiver) = d.unregister(&self.fullname) {
                // Wait up to 500ms for the daemon thread to send the goodbye packet.
                let _ = receiver.recv_timeout(std::time::Duration::from_millis(500));
            }
        }
    }
}

fn strip_service_type(fullname: &str) -> String {
    let suffix = format!(".{}", SERVICE_TYPE);
    fullname
        .strip_suffix(&suffix)
        .unwrap_or(fullname)
        .to_string()
}

fn resolve_icon(info: &ServiceInfo) -> char {
    let val = info
        .get_properties()
        .get("icon")
        .map(|r| r.val_str())
        .unwrap_or("");
    match val {
        "laptop" => icons::LAPTOP,
        "server" => icons::SERVER,
        _ => icons::DESKTOP,
    }
}
