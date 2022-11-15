use std::{
    marker::PhantomData,
    ops::{Deref, DerefMut},
};

use super::{
    neopixel::{self, Color, NeoPixel},
    Error, SeeSaw,
};
use bytes::{BufMut, BytesMut};
use embedded_hal::blocking::i2c::{Read, Write};

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
        pixel_x: usize,
        pixel_y: usize,
        color: Color,
    ) -> Result<(), Error> {
        self.0.set_pixel_color(pixel_y * 4 + pixel_x, color)
    }
}
