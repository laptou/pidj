use std::ops::{Deref, DerefMut};

use super::{
    keypad::{Edge, KeyEvent},
    neopixel::{self, Color, NeoPixel},
    Error, SeeSaw,
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
const fn neotrellis_xy(x: u16, y: u16) -> u16 {
    y * 4 + x
}

// converts neotrellis keycode into seesaw key code
const fn neotrellis_key_to_seesaw(k: u16) -> u16 {
    k / 4 * 8 + k % 4
}

// converts seesaw keycode into neotrellis key code
const fn neotrellis_key_from_seesaw(k: u16) -> u16 {
    k / 8 * 4 + k % 8
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
            .set_pixel_color(neotrellis_xy(pixel_x, pixel_y), color)
    }

    pub fn set_keypad_event(
        &mut self,
        pixel_x: u16,
        pixel_y: u16,
        edge: Edge,
        enable: bool,
    ) -> Result<(), Error> {
        self.0
            .set_keypad_event(neotrellis_key_to_seesaw(neotrellis_xy(pixel_x, pixel_y)) as u8, edge, enable)
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
            let mut evt = KeyEvent::from_u8(evt).unwrap();
            evt.key = neotrellis_key_from_seesaw(evt.key);
            evt_vec.push(evt);
        }

        Ok(evt_vec)
    }
}
