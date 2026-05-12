pub mod auth;
pub mod client;
pub mod protocol;
pub mod server;
pub mod tls;

use std::sync::{Mutex, OnceLock};

use iced::futures::channel::mpsc as ifmpsc;
use iced::futures::stream::Stream;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub enum NetEvent {
    Disconnected,
}

fn event_sink() -> &'static Mutex<Option<ifmpsc::Sender<NetEvent>>> {
    static SINK: OnceLock<Mutex<Option<ifmpsc::Sender<NetEvent>>>> = OnceLock::new();
    SINK.get_or_init(|| Mutex::new(None))
}

pub fn events() -> impl Stream<Item = NetEvent> + 'static {
    iced::stream::channel(8, |tx: ifmpsc::Sender<NetEvent>| async move {
        *event_sink().lock().unwrap() = Some(tx);
        std::future::pending::<()>().await;
    })
}

fn push_event(event: NetEvent) {
    if let Some(tx) = event_sink().lock().unwrap().as_mut() {
        let _ = tx.try_send(event);
    }
}

/// Input event transmitted over the wire.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Event {
    KeyDown(String),
    KeyUp(String),
    MouseMove { dx: i16, dy: i16 },
    MouseAbs { x: u16, y: u16 },
    MouseButton { button: u8, pressed: bool },
    Wheel { dx: i8, dy: i8 },
    KeyRepeat(String),
    MouseButtonRepeat(u8),
    Keepalive,
}

impl Event {
    pub fn encode(&self) -> Vec<u8> {
        postcard::to_allocvec(self).unwrap_or_default()
    }

    pub fn decode(buf: &[u8]) -> Option<Self> {
        postcard::from_bytes(buf).ok()
    }
}

pub use client::ClientConnection as Sender;
pub use server::ServerListener as Listener;
