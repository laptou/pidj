use std::{
    marker::PhantomData,
    ops::{Deref, DerefMut},
};

use bytes::{BufMut, BytesMut};
use embedded_hal::blocking::i2c::{Read, Write};

use super::{Error, SeeSaw};
pub use color::*;

pub const BASE: u8 = 0x0E;

pub mod functions {
    pub const PIN: u8 = 0x01;
    pub const SPEED: u8 = 0x02;
    pub const BUF_LENGTH: u8 = 0x03;
    pub const BUF: u8 = 0x04;
    pub const SHOW: u8 = 0x05;
}

pub mod color {
    use bytes::BufMut;

    pub trait ColorOrder {
        const BYTES_PER_PIXEL: u8;

        fn put(buf: &mut impl BufMut, color: Color);
    }

    #[derive(Clone, Copy)]
    pub struct RGB;

    impl ColorOrder for RGB {
        const BYTES_PER_PIXEL: u8 = 3;

        fn put(buf: &mut impl BufMut, color: Color) {
            buf.put_u8(color.r);
            buf.put_u8(color.g);
            buf.put_u8(color.b);
        }
    }

    pub struct GRB;

    impl ColorOrder for GRB {
        const BYTES_PER_PIXEL: u8 = 3;

        fn put(buf: &mut impl BufMut, color: Color) {
            buf.put_u8(color.g);
            buf.put_u8(color.r);
            buf.put_u8(color.b);
        }
    }
    pub struct RGBW;

    impl ColorOrder for RGBW {
        const BYTES_PER_PIXEL: u8 = 4;

        fn put(buf: &mut impl BufMut, color: Color) {
            buf.put_u8(color.r);
            buf.put_u8(color.g);
            buf.put_u8(color.b);
            buf.put_u8(color.w);
        }
    }
    pub struct GRBW;

    impl ColorOrder for GRBW {
        const BYTES_PER_PIXEL: u8 = 4;

        fn put(buf: &mut impl BufMut, color: Color) {
            buf.put_u8(color.g);
            buf.put_u8(color.r);
            buf.put_u8(color.b);
            buf.put_u8(color.w);
        }
    }

    #[derive(Copy, Clone, Debug, PartialEq, Eq)]

    pub struct Color {
        pub r: u8,
        pub g: u8,
        pub b: u8,
        pub w: u8,
    }
}


pub struct NeoPixel<
    I2C: Read + Write,
    S: DerefMut<Target = SeeSaw<I2C>>,
    P: ColorOrder,
    const PIXEL_COUNT: u8,
>(S, PhantomData<P>);

impl<
        I2C: Read + Write,
        S: DerefMut<Target = SeeSaw<I2C>>,
        P: ColorOrder,
        const PIXEL_COUNT: u8,
    > Deref for NeoPixel<I2C, S, P, PIXEL_COUNT>
{
    type Target = SeeSaw<I2C>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<
        I2C: Read + Write,
        S: DerefMut<Target = SeeSaw<I2C>>,
        P: ColorOrder,
        const PIXEL_COUNT: u8,
    > DerefMut for NeoPixel<I2C, S, P, PIXEL_COUNT>
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<
        I2C: Read + Write,
        S: DerefMut<Target = SeeSaw<I2C>>,
        P: ColorOrder,
        const PIXEL_COUNT: u8,
    > NeoPixel<I2C, S, P, PIXEL_COUNT>
{
    pub fn new(inner: S) -> Self {
        Self(inner, PhantomData)
    }

    pub fn init(&mut self, high_speed: bool, pin: u8) -> Result<(), Error> {
        self.write(BASE, functions::PIN, &[pin])?;
        self.write(BASE, functions::SPEED, &[high_speed as u8])?;

        let buf = u16::to_be_bytes((PIXEL_COUNT * P::BYTES_PER_PIXEL) as u16);
        self.write(BASE, functions::BUF_LENGTH, &buf[..])?;

        Ok(())
    }

    pub fn set_pixel_color(&mut self, pixel: u16, color: Color) -> Result<(), Error> {
        let mut buf = BytesMut::new();
        buf.put_u16(pixel * P::BYTES_PER_PIXEL as u16);
        P::put(&mut buf, color);
        self.write(BASE, functions::BUF, &buf[..])
    }

    pub fn show(&mut self) -> Result<(), Error> {
        self.write(BASE, functions::SHOW, &[])
    }
}
