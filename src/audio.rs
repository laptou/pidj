use std::{
    ffi::OsStr,
    fs::File,
    future::Future,
    io::BufReader,
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::Context;
use futures::stream::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use rodio::{Decoder, OutputStream, Source};
use tokio::{sync::oneshot, task::JoinHandle};
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, trace};

use crate::{
    app,
    driver::adafruit::seesaw::{keypad::Edge, neopixel::Color},
    keyboard,
};

#[derive(Debug, Clone, Copy)]
pub enum Command {}

#[derive(Debug, Clone, Copy)]
pub enum Event {}

pub async fn run(
    ct: CancellationToken,
    app_msg_tx: flume::Sender<app::Message>,
    kb_cmd_tx: flume::Sender<keyboard::Command>,
    kb_evt_rx: flume::Receiver<keyboard::Event>,
) -> anyhow::Result<()> {
    let loading_token = ct.child_token();

    let (paths, stream_handle) = {
        start_loading_animation(loading_token.clone(), kb_cmd_tx.clone());
        let _guard = loading_token.drop_guard();

        info!("locating audio files");

        let cwd = std::env::current_dir()?;
        let glob_pattern = cwd.to_string_lossy().to_string() + "/audio/**/*.{wav,flac,mp3}";

        debug!("globbing {glob_pattern:?}");

        // let pb_style = ProgressStyle::with_template(
        //     "{prefix:>12.cyan.bold} [{spinner}] {pos}/{len} {wide_msg}",
        // )?;

        // let pb = ProgressBar::new_spinner();
        // pb.set_style(pb_style);
        // pb.set_prefix("Locating");

        let paths = tokio::select! {
            _ = ct.cancelled() => { return Ok(()); }
            res = walkdir(cwd.join("audio"), &["wav", "flac", "mp3"]) => {
                res?
            }
        };

        debug!("globbed");

        // pb.finish_with_message("Located audio files");

        let (_stream, stream_handle) = OutputStream::try_default()
            .context("no audio output stream available")
            .unwrap();

        debug!("opened audio output");

        (paths, stream_handle)
    };

    info!("loaded audio files");

    loop {
        tokio::select! {
            _ = ct.cancelled() => { break; }
            kb_evt = kb_evt_rx.recv_async() => {
                match kb_evt {
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
        }
    }

    debug!("exiting audio loop");

    Ok(())
}

async fn walkdir(
    path: impl AsRef<Path>,
    extensions: &[impl AsRef<OsStr>],
) -> anyhow::Result<Vec<PathBuf>> {
    let mut current_paths = vec![path.as_ref().to_owned()];
    let mut selected_files = vec![];

    while let Some(current_path) = current_paths.pop() {
        let mut readdir = tokio::fs::read_dir(&current_path)
            .await
            .context("failed to read directory {current_path:?}")?;
        while let Some(current_file) = readdir
            .next_entry()
            .await
            .context("failed to get next file in directory {current_path:?}")?
        {
            let ft = current_file.file_type().await?;

            if ft.is_file() {
                if let Some(extension) = current_file.path().extension() {
                    for allowed_extension in extensions {
                        if allowed_extension.as_ref() == extension {
                            selected_files.push(current_file.path());
                            break;
                        }
                    }
                }
            }

            if ft.is_dir() {
                current_paths.push(current_file.path());
            }
        }
    }

    Ok(selected_files)
}

fn start_loading_animation(ct: CancellationToken, kb_cmd_tx: flume::Sender<keyboard::Command>) {
    std::thread::spawn(move || {
        debug!("initializing loading animation");

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

        while !ct.is_cancelled() {
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

            trace!("loading animation step");

            std::thread::sleep(Duration::from_millis(1000));
        }

        debug!("exiting loading animation");

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

        debug!("exited loading animation");
    });
}
