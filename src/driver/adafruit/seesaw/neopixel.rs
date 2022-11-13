use std::ops::{Deref, DerefMut};

use embedded_hal::blocking::i2c::{Read, Write};

use super::{SeeSaw, Error, SeeSawError};

pub const BASE: u8 = 0x0E;

pub mod functions {
    pub const PIN: u8 = 0x01;
    pub const SPEED: u8 = 0x02;
    pub const BUF_LENGTH: u8 = 0x03;
    pub const BUF: u8 = 0x04;
    pub const SHOW: u8 = 0x05;
}

pub const NUM_PINS: u8 = 32;
pub const MAX_BUF_BYTES: u16 = 63 * 3;

pub const MAX_BUF_WRITE_BYTES: usize = 30;
pub const MAX_RGB_WRITE_PIXELS: usize = MAX_BUF_WRITE_BYTES / 3;
pub const MAX_RGBW_WRITE_PIXELS: usize = MAX_BUF_WRITE_BYTES / 4;

#[derive(Debug, Copy, Clone)]
pub enum ColorOrder {
    RGB,
    GRB,
    RGBW,
    GRBW,
}

#[derive(Debug, Copy, Clone)]
#[repr(u8)]
pub enum Speed {
    Khz400 = 0x00,
    Khz800 = 0x01,
}

pub struct NeoPixel<I2C: Read + Write>(SeeSaw<I2C>);

impl<I2C: Read + Write> Deref for NeoPixel<I2C> {
    type Target = SeeSaw<I2C>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<I2C: Read + Write> DerefMut for NeoPixel<I2C> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<I2C: Read + Write> NeoPixel<I2C> {
    pub fn set_pin(&mut self, pin: u8) -> Result<(), Error> {
        if pin >= NUM_PINS {
            return Err(Error::SeeSaw(SeeSawError::InvalidArgument));
        }

        self.write(BASE, functions::PIN, &[pin])
    }

    pub fn set_speed(&mut self, speed: Speed) -> Result<(), Error> {
        self.write(BASE, functions::SPEED, &[speed as u8])
    }

    pub fn set_buf_length_bytes(&mut self, len: u16) -> Result<(), Error> {
        if len > MAX_BUF_BYTES {
            return Err(Error::SeeSaw(SeeSawError::InvalidArgument));
        }

        let bytes: [u8; 2] = len.to_be_bytes();
        self.write(BASE, functions::BUF_LENGTH, &bytes)
    }

    pub fn set_buf_length_pixels(&mut self, ct: usize, order: ColorOrder) -> Result<(), Error> {
        use ColorOrder::*;

        let bpp = match order {
            RGB | GRB => 3,
            RGBW | GRBW => 4,
        };

        let count = ct * bpp;

        if count <= (u16::max_value() as usize) {
            self.set_buf_length_bytes(count as u16)
        } else {
            Err(Error::SeeSaw(SeeSawError::InvalidArgument))
        }
    }

    pub fn show(&mut self) -> Result<(), Error> {
        self.write(BASE, functions::SHOW, &[])
    }

    pub fn write_buf_raw(&mut self, idx: u16, buf: &[u8]) -> Result<(), Error> {
        if buf.len() > MAX_BUF_WRITE_BYTES {
            return Err(Error::SeeSaw(SeeSawError::InvalidArgument));
        }

        let mut tx_buf = [0u8; 32];

        let tx_buf_len = 2 + buf.len();

        tx_buf[..2].copy_from_slice(&idx.to_be_bytes());
        tx_buf[2..tx_buf_len].copy_from_slice(buf);

        self.write(BASE, functions::BUF, &tx_buf[..tx_buf_len])
    }
}
