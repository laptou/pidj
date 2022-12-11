use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};

use anyhow::Context;
use embedded_hal::blocking::delay::DelayUs;
use rppal::i2c::I2c;
use tokio_util::sync::CancellationToken;
use tracing::{debug, trace};

use crate::{
    driver::{
        adafruit::seesaw::{
            keypad::Edge,
            neopixel::{Color, NeoPixel},
            neotrellis::{KeyEvent, NeoTrellis},
            SeeSaw,
        },
        ThreadDelay,
    },
    util::Interval,
};

#[derive(Debug, Clone, Copy)]
pub enum Command {
    SetState { x: u16, y: u16, state: PixelState },
}

#[derive(Debug, Clone, Copy)]
pub enum PixelState {
    Solid {
        color: Color,
        /// if true, will force a neotrellis update
        update: bool,
    },
    FadeLinear {
        from: Color,
        to: Color,
        duration: Duration,
        progress: f64,
    },
    FadeExp {
        from: Color,
        to: Color,
        duration: Duration,
        progress: f64,
    },
}

#[derive(Debug, Clone, Copy)]
pub enum Event {
    Key(KeyEvent),
}

pub fn run(
    ct: CancellationToken,
    cmd_rx: flume::Receiver<Command>,
    evt_tx: flume::Sender<Event>,
) -> anyhow::Result<()> {
    let i2c = I2c::new().context("failed to open i2c bus")?;
    let mut seesaw = SeeSaw { i2c, address: 0x2E };
    let mut delay = ThreadDelay;

    seesaw.sw_reset()?;
    let seesaw_ver = seesaw
        .get_version(&mut delay)
        .context("failed to get seesaw version")?;
    debug!("initialized adafruit seesaw driver, ver = {seesaw_ver}");

    let mut np = NeoPixel::new(&mut seesaw);
    let mut nt = NeoTrellis::new(&mut np);
    nt.init()?;

    for x in 0..4 {
        for y in 0..4 {
            nt.set_keypad_event(x, y, Edge::Rising, true)?;
            nt.set_keypad_event(x, y, Edge::Falling, true)?;
        }
    }

    debug!("initialized adafruit neotrellis driver");

    let nt = Mutex::new(nt);

    std::thread::scope(|s| {
        s.spawn({
            let nt = &nt;
            move || -> anyhow::Result<()> {
                let mut pixel_states = vec![
                    PixelState::Solid {
                        color: Color::WHITE,
                        update: true,
                    };
                    16
                ];

                let mut interval = Interval::new(Duration::from_millis(1000 / 60));

                loop {
                    interval.tick();
                    let mut nt = nt.lock().unwrap();

                    for (i, state) in pixel_states.iter_mut().enumerate() {
                        let x = (i % 4) as u16;
                        let y = (i / 4) as u16;

                        match state {
                            // solid color pixels -> do nothing
                            PixelState::Solid { color, update } => {
                                if *update {
                                    nt.set_pixel_color(x, y, *color)?;
                                    *update = false;
                                }
                            }
                            // fading pixels -> update
                            PixelState::FadeLinear {
                                from,
                                to,
                                duration,
                                progress,
                            } => {
                                *progress += duration.as_secs_f64();

                                let p = *progress;
                                let rp = 1. - p;

                                if p < 1. {
                                    let current = Color {
                                        r: (from.r as f64 * rp + to.r as f64 * p) as u8,
                                        g: (from.g as f64 * rp + to.g as f64 * p) as u8,
                                        b: (from.b as f64 * rp + to.b as f64 * p) as u8,
                                        w: (from.w as f64 * rp + to.w as f64 * p) as u8,
                                    };

                                    nt.set_pixel_color(x, y, current)?;
                                } else {
                                    nt.set_pixel_color(x, y, *to)?;
                                    *state = PixelState::Solid {
                                        color: *to,
                                        update: true,
                                    };
                                }
                            }
                            PixelState::FadeExp {
                                from,
                                to,
                                duration,
                                progress,
                            } => {
                                *progress += duration.as_secs_f64();

                                let p = *progress;
                                let p = p * p * p;
                                let rp = 1. - p;

                                if p < 1. {
                                    let current = Color {
                                        r: (from.r as f64 * rp + to.r as f64 * p) as u8,
                                        g: (from.g as f64 * rp + to.g as f64 * p) as u8,
                                        b: (from.b as f64 * rp + to.b as f64 * p) as u8,
                                        w: (from.w as f64 * rp + to.w as f64 * p) as u8,
                                    };

                                    nt.set_pixel_color(x, y, current)?;
                                } else {
                                    *state = PixelState::Solid {
                                        color: *to,
                                        update: true,
                                    };
                                }
                            }
                        }
                    }

                    match cmd_rx.try_recv() {
                        Ok(mut cmd) => {
                            // then pull all of the pending commands out of the channel and
                            // execute them
                            loop {
                                trace!("executing command {cmd:?}");

                                match cmd {
                                    Command::SetState { x, y, state } => {
                                        let i = (y * 4 + x) as usize;
                                        pixel_states[i] = state;
                                    }
                                }

                                cmd = match cmd_rx.try_recv() {
                                    Ok(cmd) => cmd,
                                    Err(_) => break,
                                };
                            }
                        }
                        Err(flume::TryRecvError::Empty) => {
                            // there are no commands, so we just keep going
                        }
                        Err(flume::TryRecvError::Disconnected) => break,
                    };

                    // tokio::time::sleep(Duration::from_micros(300)).await;
                    nt.show()?;
                }

                debug!("exiting keyboard loop");

                Ok(())
            }
        });

        s.spawn({
            let nt = &nt;
            move || -> anyhow::Result<()> {
                // sample keyboard for events at 120Hz

                let mut interval = Interval::new(Duration::from_millis(1000 / 120));

                loop {
                    interval.tick();
                    let mut nt = nt.lock().unwrap();

                    for evt in nt.get_keypad_events(&mut delay)? {
                        trace!("received event {evt:?}");
                        let _ = evt_tx.send(Event::Key(evt));
                    }
                }
            }
        });
    });

    Ok(())
}
