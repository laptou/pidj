//! Driver for the Adafruit Seesaw.
//! Based on https://github.com/ferrous-systems/adafruit-seesaw/blob/main/src/lib.rs.

use embedded_hal::blocking::{
    delay::DelayUs,
    i2c::{Read, Write},
};
use thiserror::Error;
use tracing::info;

pub struct SeeSaw<I2C> {
    pub i2c: I2C,
    pub address: u8,
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("i2c error")]
    I2c,
    #[error("seesaw protocol error")]
    SeeSaw(#[from] SeeSawError),
}

#[derive(Debug, Error)]
pub enum SeeSawError {
    #[error("invalid size")]
    InvalidSize,
    #[error("invalid argument")]
    InvalidArgument,
}

const BUFFER_MAX: usize = 32;
const PAYLOAD_MAX: usize = BUFFER_MAX - 2;

pub mod keypad;
pub mod neopixel;
pub mod neotrellis;
pub mod status;

impl<I2C> SeeSaw<I2C>
where
    I2C: Read + Write,
{
    fn write(&mut self, base: u8, function: u8, buf: &[u8]) -> Result<(), Error> {
        if buf.len() > PAYLOAD_MAX {
            info!("payload max!");
            return Err(Error::SeeSaw(SeeSawError::InvalidSize));
        }

        let mut tx_buf: [u8; 32] = [0u8; 32];

        let end = 2 + buf.len();

        tx_buf[0] = base;
        tx_buf[1] = function;
        tx_buf[2..end].copy_from_slice(buf);

        self.i2c
            .write(self.address, &tx_buf[..end])
            .map_err(|_| Error::I2c)
    }

    fn read<DELAY: DelayUs<u32>>(
        &mut self,
        base: u8,
        function: u8,
        delay: &mut DELAY,
        buf: &mut [u8],
    ) -> Result<(), Error> {
        self.write(base, function, &[])?;
        delay.delay_us(14000);
        self.i2c.read(self.address, buf).map_err(|_| Error::I2c)
    }

    pub fn sw_reset(&mut self) -> Result<(), Error> {
        self.write(status::BASE, status::functions::SWRST, &[0xFF])
    }

    /// Get the count of pending key events on the keypad
    pub fn get_keypad_event_count<DELAY: DelayUs<u32>>(
        &mut self,
        delay: &mut DELAY,
    ) -> Result<u8, Error> {
        let mut buf = [0u8; 1];
        self.read(keypad::BASE, keypad::functions::COUNT, delay, &mut buf)?;
        Ok(buf[0])
    }

    /// Enable or disable the interrupt
    pub fn set_keypad_interrupt(&mut self, enable: bool) -> Result<(), Error> {
        use keypad::functions::{INTENCLR, INTENSET};

        let func = if enable { INTENSET } else { INTENCLR };
        self.write(keypad::BASE, func, &[1])
    }

    /// Set or clear the trigger event on a given key.
    pub fn set_keypad_event(
        &mut self,
        key: u8,
        edge: keypad::Edge,
        status: keypad::Status,
    ) -> Result<(), Error> {
        let stat: u8 = (1 << ((edge as u8) + 1)) | (status as u8);
        self.write(keypad::BASE, keypad::functions::EVENT, &[key, stat])
    }

    /// Read an event on a given key.
    pub fn get_keypad_events<DELAY: DelayUs<u32>>(
        &mut self,
        buf: &mut [u8],
        delay: &mut DELAY,
    ) -> Result<(), Error> {
        self.read(keypad::BASE, keypad::functions::FIFO, delay, buf)
    }

    pub fn get_status_hwid<DELAY: DelayUs<u32>>(&mut self, delay: &mut DELAY) -> Result<u8, Error> {
        let mut buf = [0u8; 1];
        self.read(status::BASE, status::functions::HW_ID, delay, &mut buf)
            .map_err(|_| Error::I2c)?;
        Ok(buf[0])
    }

    pub fn get_version<DELAY: DelayUs<u32>>(
        &mut self,
        delay: &mut DELAY,
    ) -> Result<u32, Error> {
        let mut buf = [0u8; 4];
        self.read(status::BASE, status::functions::VERSION, delay, &mut buf)
            .map_err(|_| Error::I2c)?;
        Ok(u32::from_be_bytes(buf))
    }

    pub fn get_options<DELAY: DelayUs<u32>>(
        &mut self,
        delay: &mut DELAY,
    ) -> Result<u32, Error> {
        let mut buf = [0u8; 4];
        self.read(status::BASE, status::functions::OPTIONS, delay, &mut buf)
            .map_err(|_| Error::I2c)?;
        Ok(u32::from_be_bytes(buf))
    }

    /// Get temperature in Celsius.
    pub fn get_temp<DELAY: DelayUs<u32>>(
        &mut self,
        delay: &mut DELAY,
    ) -> Result<u32, Error> {
        let mut buf = [0u8; 4];
        self.read(status::BASE, status::functions::TEMP, delay, &mut buf)
            .map_err(|_| Error::I2c)?;
        Ok(u32::from_be_bytes(buf) / (1 << 16))
    }
}
