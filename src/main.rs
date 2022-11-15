use std::time::Duration;

use anyhow::Context;

use driver::adafruit::seesaw::SeeSaw;
use embedded_hal::prelude::_embedded_hal_blocking_delay_DelayUs;

use rppal::i2c::I2c;

use crate::driver::adafruit::seesaw::{
    keypad::Edge,
    neopixel::{Color, NeoPixel},
    neotrellis::NeoTrellis,
};
mod driver;

struct Delay;

impl embedded_hal::blocking::delay::DelayUs<u32> for Delay {
    fn delay_us(&mut self, us: u32) {
        std::thread::sleep(Duration::from_micros(us as u64))
    }
}

fn main() -> anyhow::Result<()> {
    let i2c = I2c::new().context("failed to open i2c bus")?;
    let mut seesaw = SeeSaw { i2c, address: 0x2E };
    let mut delay = Delay;

    seesaw.sw_reset()?;
    let seesaw_ver = seesaw
        .get_version(&mut delay)
        .context("failed to get seesaw version")?;
    println!("seesaw version: {seesaw_ver}");

    let mut np = NeoPixel::new(&mut seesaw);
    let mut nt = NeoTrellis::new(&mut np);
    nt.init()?;

    for x in 0..4 {
        for y in 0..4 {
            nt.set_keypad_event(x, y, Edge::Rising, true)?;
        }
    }

    loop {
        for x in 0..4 {
            for y in 0..4 {
                nt.set_pixel_color(
                    x,
                    y,
                    Color {
                        r: (x * 85) as u8,
                        g: (y * 85) as u8,
                        b: 0,
                        w: 0,
                    },
                )?;
            }
        }

        delay.delay_us(300);

        nt.show()?;

        for evt in nt.get_keypad_events(&mut delay)? {
            println!("key event: {evt:?}");
        }

        delay.delay_us(300);
    }
}
