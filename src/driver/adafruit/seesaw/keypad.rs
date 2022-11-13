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
    pub key: u8,
    pub event: Edge,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
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

impl Edge {
    pub fn from_u8(val: u8) -> Result<Self, super::Error> {
        match val {
            0 => Ok(Edge::High),
            1 => Ok(Edge::Low),
            2 => Ok(Edge::Falling),
            3 => Ok(Edge::Rising),
            _ => Err(super::Error::SeeSaw(super::SeeSawError::InvalidArgument)),
        }
    }
}

#[repr(u8)]
pub enum Status {
    Disable = 0x00,
    Enable = 0x01,
}
