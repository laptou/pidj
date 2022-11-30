use std::ops::{Deref, DerefMut};

use super::{
    keypad::Edge,
    neopixel::{self, Color, NeoPixel},
    Error, SeeSaw, SeeSawError,
};
use bytes::{Buf, BytesMut};
use embedded_hal::blocking::{
    delay::DelayUs,
    i2c::{Read, Write},
};
use num_traits::FromPrimitive;

pub struct NeoTrellis<
    I2C: Read + Write,
    S: DerefMut<Target = SeeSaw<I2C>>,
    NP: DerefMut<Target = NeoPixel<I2C, S, neopixel::GRB, 16>>,
>(NP);

impl<
        I2C: Read + Write,
        S: DerefMut<Target = SeeSaw<I2C>>,
        NP: DerefMut<Target = NeoPixel<I2C, S, neopixel::GRB, 16>>,
    > Deref for NeoTrellis<I2C, S, NP>
{
    type Target = NeoPixel<I2C, S, neopixel::GRB, 16>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<
        I2C: Read + Write,
        S: DerefMut<Target = SeeSaw<I2C>>,
        NP: DerefMut<Target = NeoPixel<I2C, S, neopixel::GRB, 16>>,
    > DerefMut for NeoTrellis<I2C, S, NP>
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

// converts x and y into a neotrellis key code
pub const fn neotrellis_xy_to_key(x: u16, y: u16) -> u16 {
    y * 4 + x
}

// converts x and y into a neotrellis key code
pub const fn neotrellis_key_to_xy(k: u16) -> (u16, u16) {
    (k / 4, k % 4)
}

// converts neotrellis keycode into seesaw key code
const fn neotrellis_key_to_seesaw(k: u16) -> u16 {
    k / 4 * 8 + k % 4
}

// converts seesaw keycode into neotrellis key code
const fn neotrellis_key_from_seesaw(k: u16) -> u16 {
    k / 8 * 4 + k % 8
}

/// This is a NeoTrellis key event. This differs from
/// [`super::keypad::KeyEvent`] because it represents a key as (x, y) instead of
/// as a key code. Creating this from a [`super::keypad::KeyEvent`] also
/// implicitly converts the seesaw keycode into a neotrellis keycode.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct KeyEvent {
    pub key: (u16, u16),
    pub edge: Edge,
}

impl From<super::keypad::KeyEvent> for KeyEvent {
    fn from(kev: super::keypad::KeyEvent) -> Self {
        Self {
            key: neotrellis_key_to_xy(neotrellis_key_from_seesaw(kev.key)),
            edge: kev.edge,
        }
    }
}

impl Into<super::keypad::KeyEvent> for KeyEvent {
    fn into(self) -> super::keypad::KeyEvent {
        super::keypad::KeyEvent {
            key: neotrellis_key_to_seesaw(neotrellis_xy_to_key(self.key.0, self.key.1)),
            edge: self.edge,
        }
    }
}

impl FromPrimitive for KeyEvent {
    fn from_i64(n: i64) -> Option<Self> {
        Some(super::keypad::KeyEvent::from_i64(n)?.into())
    }

    fn from_u64(n: u64) -> Option<Self> {
        Some(super::keypad::KeyEvent::from_u64(n)?.into())
    }
}

impl<
        I2C: Read + Write,
        S: DerefMut<Target = SeeSaw<I2C>>,
        NP: DerefMut<Target = NeoPixel<I2C, S, neopixel::GRB, 16>>,
    > NeoTrellis<I2C, S, NP>
{
    pub fn new(inner: NP) -> Self {
        Self(inner)
    }

    pub fn init(&mut self) -> Result<(), Error> {
        // NeoTrellis pin is 3
        self.0.init(true, 3)
    }

    pub fn set_pixel_color(
        &mut self,
        pixel_x: u16,
        pixel_y: u16,
        color: Color,
    ) -> Result<(), Error> {
        self.0
            .set_pixel_color(neotrellis_xy_to_key(pixel_x, pixel_y), color)
    }

    pub fn set_keypad_event(
        &mut self,
        pixel_x: u16,
        pixel_y: u16,
        edge: Edge,
        enable: bool,
    ) -> Result<(), Error> {
        self.0.set_keypad_event(
            neotrellis_key_to_seesaw(neotrellis_xy_to_key(pixel_x, pixel_y)) as u8,
            edge,
            enable,
        )
    }

    pub fn get_keypad_events<DELAY: DelayUs<u32>>(
        &mut self,
        delay: &mut DELAY,
    ) -> Result<Vec<KeyEvent>, Error> {
        let evt_count = self.0.get_keypad_event_count(delay)? as usize;
        if evt_count == 0 {
            return Ok(Vec::new());
        }

        let mut evt_buf = BytesMut::zeroed(evt_count + 2);
        let mut evt_vec = Vec::new();
        self.0.get_keypad_events_raw(&mut evt_buf[..], delay)?;

        for _ in 0..evt_count {
            let evt = evt_buf.get_u8();
            let evt = KeyEvent::from_u8(evt).ok_or(Error::SeeSaw(SeeSawError::InvalidKeycode))?;

            if evt.key.0 > 3 || evt.key.1 > 3 {
                // tiled neotrellis not supported
                continue;
            }

            evt_vec.push(evt);
        }

        Ok(evt_vec)
    }
}
