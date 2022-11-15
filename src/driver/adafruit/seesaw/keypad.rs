use num_derive::{FromPrimitive, ToPrimitive};
use num_traits::FromPrimitive;

pub const BASE: u8 = 0x10;

pub mod functions {
    pub const STATUS: u8 = 0x00;
    pub const EVENT: u8 = 0x01;
    pub const INTENSET: u8 = 0x02;
    pub const INTENCLR: u8 = 0x03;
    pub const COUNT: u8 = 0x04;
    pub const FIFO: u8 = 0x10;
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct KeyEvent {
    pub key: u16,
    pub edge: Edge,
}

impl FromPrimitive for KeyEvent {
    fn from_i64(_: i64) -> Option<Self> {
        None
    }

    fn from_u64(n: u64) -> Option<Self> {
        Some(Self {
            edge: Edge::from_u8((n & 0b11) as u8)?,
            key: ((n & 0b1111_1111_1111_1100) >> 2) as u16,
        })
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, FromPrimitive, ToPrimitive)]
#[repr(u8)]
pub enum Edge {
    /// Indicates that the key is currently pressed
    High = 0x00,

    /// Indicates that the key is currently released
    Low = 0x01,

    /// Indicates that the key was recently released
    Falling = 0x02,

    /// Indicates that the key was recently pressed
    Rising = 0x03,
}
