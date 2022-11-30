use std::{
    collections::HashMap,
    thread::JoinHandle,
    time::{Duration, Instant}, sync::Arc,
};

use anyhow::Context;
use cancellation::CancellationToken;
use embedded_hal::blocking::delay::DelayUs;
use rppal::i2c::I2c;
use tracing::{debug, trace};

use crate::{
    driver::adafruit::seesaw::{
        keypad::Edge,
        neopixel::{Color, NeoPixel},
        neotrellis::{KeyEvent, NeoTrellis},
        SeeSaw,
    },
    keyboard,
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
        progress: f32,
    },
    FadeExp {
        from: Color,
        to: Color,
        duration: Duration,
        progress: f32,
    },
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

pub fn spawn_thread(
    ct: Arc<CancellationToken>,
    cmd_rx: flume::Receiver<Command>,
    evt_tx: flume::Sender<Event>,
) -> JoinHandle<anyhow::Result<()>> {
    std::thread::spawn(move || {
        let i2c = I2c::new().context("failed to open i2c bus")?;
        let mut seesaw = SeeSaw { i2c, address: 0x2E };
        let mut delay = Delay;

        seesaw.sw_reset()?;
        let seesaw_ver = seesaw
            .get_version(&mut delay)
            .context("failed to get seesaw version")?;
        debug!("initialized adafruit seesaw driver, seesaw version = {seesaw_ver}");

        let mut np = NeoPixel::new(&mut seesaw);
        let mut nt = NeoTrellis::new(&mut np);
        nt.init()?;
        debug!("initialized adafruit neotrellis driver");

        let mut pixel_states = HashMap::new();

        for x in 0..4 {
            for y in 0..4 {
                pixel_states.insert(
                    (x, y),
                    PixelState::Solid {
                        color: Color::WHITE,
                        update: true,
                    },
                );
                nt.set_keypad_event(x, y, Edge::Rising, true)?;
                nt.set_keypad_event(x, y, Edge::Falling, true)?;
            }
        }

        let frame_duration = Duration::from_millis(1000 / 60);
        let mut last_frame = Instant::now();

        while !ct.is_canceled() {
            let current_frame = Instant::now();
            let frame_time = current_frame - last_frame;
            last_frame = current_frame;

            for (&(x, y), state) in pixel_states.iter_mut() {
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
                        let delta_progress =
                            frame_time.as_micros() as f32 / duration.as_micros() as f32;
                        *progress += delta_progress;

                        let p = *progress;
                        let rp = 1. - p;

                        if p < 1. {
                            let current = Color {
                                r: (from.r as f32 * rp + to.r as f32 * p) as u8,
                                g: (from.g as f32 * rp + to.g as f32 * p) as u8,
                                b: (from.b as f32 * rp + to.b as f32 * p) as u8,
                                w: (from.w as f32 * rp + to.w as f32 * p) as u8,
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
                        let delta_progress =
                            frame_time.as_micros() as f32 / duration.as_micros() as f32;
                        *progress += delta_progress;

                        let p = *progress;
                        let p = p * p * p;
                        let rp = 1. - p;

                        if p < 1. {
                            let current = Color {
                                r: (from.r as f32 * rp + to.r as f32 * p) as u8,
                                g: (from.g as f32 * rp + to.g as f32 * p) as u8,
                                b: (from.b as f32 * rp + to.b as f32 * p) as u8,
                                w: (from.w as f32 * rp + to.w as f32 * p) as u8,
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

            // idle until we get a new command
            // but timeout at 20ms so we can check for keyboard events
            match cmd_rx.recv_timeout(Duration::from_millis(10)) {
                Ok(mut cmd) => {
                    // then pull all of the pending commands out of the channel and
                    // execute them
                    loop {
                        trace!("executing command {cmd:?}");

                        match cmd {
                            Command::SetState { x, y, state } => {
                                pixel_states.entry((x, y)).and_modify(|s| *s = state);
                            }
                        }

                        cmd = match cmd_rx.try_recv() {
                            Ok(cmd) => cmd,
                            Err(_) => break,
                        };
                    }
                }
                Err(flume::RecvTimeoutError::Timeout) => {
                    // timed out, there are no commands, so we just keep going
                }
                Err(flume::RecvTimeoutError::Disconnected) => break,
            };

            delay.delay_us(300);

            nt.show()?;

            for evt in nt.get_keypad_events(&mut delay)? {
                trace!("received event {evt:?}");

                let _ = evt_tx.send(Event::Key(evt));
            }

            if frame_duration > frame_time {
                std::thread::sleep(frame_duration - frame_time);
            }
        }

        for x in 0..4 {
            for y in 0..4 {
                nt.set_pixel_color(x, y, Color::BLACK)?;
            }
        }

        nt.show()?;

        debug!("exiting keyboard loop");

        Ok(())
    })
}
