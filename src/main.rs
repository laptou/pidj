use std::time::Duration;

use anyhow::Context;
use driver::adafruit::seesaw::SeeSaw;
use embedded_hal::prelude::_embedded_hal_blocking_delay_DelayUs;
use rppal::i2c::I2c;

use crate::driver::adafruit::seesaw::neopixel::{Color, NeoPixel, GRB};
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

    let mut np = NeoPixel::<_, _, GRB, 16>::new(&mut seesaw);
    np.init(true, 3)?;

    loop {
        np.set_pixel_color(
            1,
            Color {
                r: 255,
                g: 0,
                b: 0,
                w: 255,
            },
        )?;
        delay.delay_us(300);
        np.show()?;
    }

    // std::thread::sleep(Duration::from_secs(5));

    Ok(())
}
