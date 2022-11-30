use std::{thread::JoinHandle, time::Duration};

use anyhow::Context;
use embedded_hal::prelude::_embedded_hal_blocking_delay_DelayUs;
use rppal::i2c::I2c;
use tokio::sync::{broadcast, mpsc};

use crate::driver::adafruit::seesaw::{
    keypad::Edge,
    neopixel::{Color, NeoPixel},
    neotrellis::{KeyEvent, NeoTrellis},
    SeeSaw,
};

#[derive(Debug, Clone, Copy)]
pub enum Command {
    UpdateColor { x: u16, y: u16, color: Color },
}

#[derive(Debug, Clone, Copy)]
pub enum Event {
    Key(KeyEvent),
}

struct Delay;

impl embedded_hal::blocking::delay::DelayUs<u32> for Delay {
    fn delay_us(&mut self, us: u32) {
        std::thread::sleep(Duration::from_micros(us as u64))
    }
}

pub fn spawn_keyboard_thread(
    mut cmd_rx: mpsc::Receiver<Command>,
    evt_tx: broadcast::Sender<Event>,
) -> JoinHandle<anyhow::Result<()>> {
    std::thread::spawn(move || {
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
            // idle until we get a new command
            let mut cmd = match cmd_rx.blocking_recv() {
                Some(cmd) => cmd,
                None => break,
            };

            // then pull all of the pending commands out of the channel and
            // execute them
            loop {
                match cmd {
                    Command::UpdateColor { x, y, color } => nt.set_pixel_color(x, y, color)?,
                }

                cmd = match cmd_rx.try_recv() {
                    Ok(cmd) => cmd,
                    Err(_) => break,
                };
            }

            delay.delay_us(300);

            nt.show()?;

            for evt in nt.get_keypad_events(&mut delay)? {
                let _ = evt_tx.send(Event::Key(evt));
            }

            delay.delay_us(300);
        }

        Ok(())
    })
}
