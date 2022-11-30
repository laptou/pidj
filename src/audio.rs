use std::{
    collections::HashMap,
    fs::File,
    io::BufReader,
    sync::{atomic::AtomicBool, Arc},
    thread::JoinHandle,
    time::{Duration, Instant},
};

use anyhow::Context;
use cancellation::{CancellationToken, CancellationTokenSource};
use embedded_hal::blocking::can;
use indicatif::{ParallelProgressIterator, ProgressBar, ProgressIterator, ProgressStyle};
use palette::encoding::pixel;
use rayon::prelude::*;
use rodio::{Decoder, OutputStream, Source};
use tracing::{debug, info, trace};

use crate::{
    driver::adafruit::seesaw::{keypad::Edge, neopixel::Color},
    keyboard,
};

#[derive(Debug, Clone, Copy)]
pub enum Command {}

#[derive(Debug, Clone, Copy)]
pub enum Event {}

pub fn spawn_thread(
    ct: Arc<CancellationToken>,
    kb_cmd_tx: flume::Sender<keyboard::Command>,
    kb_evt_rx: flume::Receiver<keyboard::Event>,
) -> JoinHandle<anyhow::Result<()>> {
    std::thread::spawn(move || {
        let cts = CancellationTokenSource::new();
        start_loading_animation(kb_cmd_tx.clone(), cts.token().clone());

        info!("locating audio files");

        let cwd = std::env::current_dir()?;
        let glob_pattern = cwd.to_string_lossy() + "/**/*.{wav,flac,mp3}";

        debug!("globbing {glob_pattern:?}");

        let pb_style = ProgressStyle::with_template(
            "{prefix:>12.cyan.bold} [{spinner}] {pos}/{len} {wide_msg}",
        )
        .unwrap();

        let pb = ProgressBar::new_spinner();
        pb.set_style(pb_style);
        pb.set_prefix("Locating");

        let paths = globwalk::glob(glob_pattern)?
            .map(|entry| -> anyhow::Result<_> {
                let entry = entry?;
                let path = entry.path();
                pb.set_message(path.to_string_lossy().to_string());
                Ok(path.to_path_buf())
            })
            .collect::<Result<Vec<_>, _>>()
            .context("failed to locate audio files")?;

        pb.finish_with_message("Located audio files");

        let (_stream, stream_handle) =
            OutputStream::try_default().context("no audio output stream available")?;

        cts.cancel();

        while !ct.is_canceled() {
            match kb_evt_rx.recv() {
                Ok(evt) => match evt {
                    keyboard::Event::Key(evt) => {
                        if let Edge::Rising = evt.edge {
                            let sound_idx = (evt.key.0 * 4 + evt.key.1) as usize;

                            let path = match paths.get(sound_idx) {
                                Some(path) => path,
                                None => continue,
                            };

                            debug!("key {:?} pressed, playing sound from {path:?}", evt.key);

                            let file = File::open(path).context("failed to open audio file")?;
                            let reader = BufReader::new(file);
                            let decoder =
                                Decoder::new(reader).context("failed to decode audio file")?;

                            stream_handle
                                .play_raw(decoder.convert_samples())
                                .context("failed to play sound")?;

                            let _ = kb_cmd_tx.send(keyboard::Command::SetState {
                                x: evt.key.0,
                                y: evt.key.1,
                                state: keyboard::PixelState::FadeExp {
                                    from: Color::from_f32(1., 0., 0.),
                                    to: Color::WHITE,
                                    duration: Duration::from_secs(1),
                                    progress: 0.,
                                },
                            });
                        }
                    }
                },

                Err(_) => break,
            }
        }

        debug!("exiting audio loop");

        Ok(())
    })
}

fn start_loading_animation(
    kb_cmd_tx: flume::Sender<keyboard::Command>,
    cancel: Arc<CancellationToken>,
) {
    std::thread::spawn(move || {
        for x in 0..4 {
            for y in 0..4 {
                let _ = kb_cmd_tx.send(keyboard::Command::SetState {
                    x,
                    y,
                    state: keyboard::PixelState::Solid {
                        color: Color::from_f32(0., 0., 0.3),
                        update: true,
                    },
                });
            }
        }

        let mut highlight = 0;

        while !cancel.is_canceled() {
            let x = highlight % 4;
            let y = highlight / 4;

            let _ = kb_cmd_tx.send(keyboard::Command::SetState {
                x,
                y,
                state: keyboard::PixelState::Solid {
                    color: Color::from_f32(0., 0.1, 0.3),
                    update: true,
                },
            });

            highlight = (highlight + 1) % 16;

            let x = highlight % 4;
            let y = highlight / 4;

            let _ = kb_cmd_tx.send(keyboard::Command::SetState {
                x,
                y,
                state: keyboard::PixelState::Solid {
                    color: Color::from_f32(0., 0.2, 0.7),
                    update: true,
                },
            });

            std::thread::sleep(Duration::from_millis(250));
        }

        for x in 0..4 {
            for y in 0..4 {
                let _ = kb_cmd_tx.send(keyboard::Command::SetState {
                    x,
                    y,
                    state: keyboard::PixelState::Solid {
                        color: Color::WHITE,
                        update: true,
                    },
                });
            }
        }
    });
}
