use std::time::Duration;

use anyhow::Context;

use driver::adafruit::seesaw::SeeSaw;
use embedded_hal::prelude::_embedded_hal_blocking_delay_DelayUs;

use palette::{rgb::Rgb, FromColor, Hsv};
use rppal::i2c::I2c;
use tokio::sync::{broadcast, mpsc};

use crate::driver::adafruit::seesaw::{
    keypad::Edge,
    neopixel::{Color, NeoPixel},
    neotrellis::NeoTrellis,
};
mod driver;
mod keyboard;

fn main() -> anyhow::Result<()> {
    let (cmd_tx, cmd_rx) = mpsc::channel(256);
    let (evt_tx, evt_rx) = broadcast::channel(256);
    let kb_join = keyboard::spawn_keyboard_thread(cmd_rx, evt_tx);

    kb_join.join().unwrap()?;

    Ok(())
}
