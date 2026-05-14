pub mod auth;
pub mod client;
pub mod protocol;
pub mod server;
pub mod tls;

use std::sync::{Mutex, OnceLock};

use iced::futures::channel::mpsc as ifmpsc;
use iced::futures::stream::Stream;

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

/// Wire tag values for the compact event encoding.
const TAG_KEY_DOWN: u8 = 0x01;
const TAG_KEY_UP: u8 = 0x02;
const TAG_KEY_REPEAT: u8 = 0x03;
const TAG_MOUSE_MOVE: u8 = 0x04;
const TAG_MOUSE_ABS: u8 = 0x05;
const TAG_MOUSE_BUTTON: u8 = 0x06;
const TAG_MOUSE_BUTTON_REPEAT: u8 = 0x07;
const TAG_WHEEL: u8 = 0x08;
const TAG_KEEPALIVE: u8 = 0x09;

/// Input event transmitted over the wire.
///
/// Uses a compact 5-byte fixed encoding per event.
#[derive(Debug, Clone)]
pub enum Event {
    KeyDown(u16),
    KeyUp(u16),
    MouseMove { dx: i16, dy: i16 },
    MouseAbs { x: u16, y: u16 },
    MouseButton { button: u8, pressed: bool },
    Wheel { dx: i8, dy: i8 },
    KeyRepeat(u16),
    MouseButtonRepeat(u8),
    Keepalive,
}

impl Event {
    /// Fixed size of a single encoded event in bytes.
    pub const ENCODED_SIZE: usize = 5;

    /// Encode a single event into a fixed 5-byte buffer.
    pub fn encode(&self) -> [u8; Self::ENCODED_SIZE] {
        let mut buf = [0u8; Self::ENCODED_SIZE];
        match self {
            Event::KeyDown(code) => {
                buf[0] = TAG_KEY_DOWN;
                buf[1..3].copy_from_slice(&code.to_le_bytes());
            }
            Event::KeyUp(code) => {
                buf[0] = TAG_KEY_UP;
                buf[1..3].copy_from_slice(&code.to_le_bytes());
            }
            Event::KeyRepeat(code) => {
                buf[0] = TAG_KEY_REPEAT;
                buf[1..3].copy_from_slice(&code.to_le_bytes());
            }
            Event::MouseMove { dx, dy } => {
                buf[0] = TAG_MOUSE_MOVE;
                buf[1..3].copy_from_slice(&dx.to_le_bytes());
                buf[3..5].copy_from_slice(&dy.to_le_bytes());
            }
            Event::MouseAbs { x, y } => {
                buf[0] = TAG_MOUSE_ABS;
                buf[1..3].copy_from_slice(&x.to_le_bytes());
                buf[3..5].copy_from_slice(&y.to_le_bytes());
            }
            Event::MouseButton { button, pressed } => {
                buf[0] = TAG_MOUSE_BUTTON;
                buf[1] = *button | (if *pressed { 0x80 } else { 0 });
            }
            Event::Wheel { dx, dy } => {
                buf[0] = TAG_WHEEL;
                buf[1] = *dx as u8;
                buf[2] = *dy as u8;
            }
            Event::MouseButtonRepeat(button) => {
                buf[0] = TAG_MOUSE_BUTTON_REPEAT;
                buf[1] = *button;
            }
            Event::Keepalive => {
                buf[0] = TAG_KEEPALIVE;
            }
        }
        buf
    }

    /// Decode a single event from the start of a byte slice.
    /// Returns `None` if the slice is too short or the tag is unknown.
    pub fn decode(buf: &[u8]) -> Option<Self> {
        if buf.len() < Self::ENCODED_SIZE {
            return None;
        }
        let tag = buf[0];
        let event = match tag {
            TAG_KEY_DOWN => Event::KeyDown(u16::from_le_bytes([buf[1], buf[2]])),
            TAG_KEY_UP => Event::KeyUp(u16::from_le_bytes([buf[1], buf[2]])),
            TAG_KEY_REPEAT => Event::KeyRepeat(u16::from_le_bytes([buf[1], buf[2]])),
            TAG_MOUSE_MOVE => Event::MouseMove {
                dx: i16::from_le_bytes([buf[1], buf[2]]),
                dy: i16::from_le_bytes([buf[3], buf[4]]),
            },
            TAG_MOUSE_ABS => Event::MouseAbs {
                x: u16::from_le_bytes([buf[1], buf[2]]),
                y: u16::from_le_bytes([buf[3], buf[4]]),
            },
            TAG_MOUSE_BUTTON => Event::MouseButton {
                button: buf[1] & 0x7F,
                pressed: (buf[1] & 0x80) != 0,
            },
            TAG_WHEEL => Event::Wheel {
                dx: buf[1] as i8,
                dy: buf[2] as i8,
            },
            TAG_MOUSE_BUTTON_REPEAT => Event::MouseButtonRepeat(buf[1]),
            TAG_KEEPALIVE => Event::Keepalive,
            _ => return None,
        };
        Some(event)
    }

    /// Encode a batch of events into a Vec.
    /// Format: `[count: u8][event0: 5][event1: 5]...`
    pub fn encode_batch(events: &[Event]) -> Vec<u8> {
        let count = events.len().min(u8::MAX as usize) as u8;
        let mut buf = Vec::with_capacity(1 + count as usize * Self::ENCODED_SIZE);
        buf.push(count);
        for event in events.iter().take(count as usize) {
            buf.extend_from_slice(&event.encode());
        }
        buf
    }

    /// Decode a batch of events from a byte slice.
    /// Returns the decoded events and the number of bytes consumed.
    pub fn decode_batch(buf: &[u8]) -> Option<(Vec<Event>, usize)> {
        if buf.is_empty() {
            return None;
        }
        let count = buf[0] as usize;
        let expected = 1 + count * Self::ENCODED_SIZE;
        if buf.len() < expected {
            return None;
        }
        let mut events = Vec::with_capacity(count);
        for i in 0..count {
            let offset = 1 + i * Self::ENCODED_SIZE;
            let event = Self::decode(&buf[offset..offset + Self::ENCODED_SIZE])?;
            events.push(event);
        }
        Some((events, expected))
    }
}

pub use client::ClientConnection as Sender;
pub use server::ServerListener as Listener;
