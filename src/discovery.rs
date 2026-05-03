use std::sync::OnceLock;

use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};

use crate::config::ServerIcon;
use crate::icons;

const SERVICE_TYPE: &str = "_spud._tcp.local.";

#[derive(Debug, Clone)]
pub struct DiscoveredServer {
    pub fullname: String,
    pub name: String,
    pub host: String,
    pub port: String,
    pub address: String,
    pub icon: char,
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
                    let host = info.get_hostname().trim_end_matches('.').to_string();
                    let port = info.get_port().to_string();
                    let icon = resolve_icon(&info);
                    let address = format!("{}:{}", host, port);
                    let _ = tx.try_send(Event::Found(DiscoveredServer {
                        fullname,
                        name,
                        host,
                        port,
                        address,
                        icon,
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
    pub fn new(name: &str, port: u16, icon: ServerIcon) -> Option<Self> {
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
        let instance_name = format!("{}-{}-{}", name, port, std::process::id());
        let info = ServiceInfo::new(SERVICE_TYPE, &instance_name, &fqdn, (), port, Some(props))
            .map_err(|e| eprintln!("[spud] mDNS ServiceInfo: {e}"))
            .ok()?
            .enable_addr_auto();
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
            let _ = d.unregister(&self.fullname);
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
