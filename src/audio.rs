use std::{fs::File, io::BufReader, thread::JoinHandle, time::Duration};

use anyhow::Context;
use indicatif::{ParallelProgressIterator, ProgressBar, ProgressIterator, ProgressStyle};
use rayon::prelude::*;
use rodio::{Decoder, OutputStream, Source};
use tracing::{debug, info, trace};

use crate::{driver::adafruit::seesaw::keypad::Edge, keyboard};

#[derive(Debug, Clone, Copy)]
pub enum Command {}

#[derive(Debug, Clone, Copy)]
pub enum Event {}

pub fn spawn_thread(kb_evt_rx: flume::Receiver<keyboard::Event>) -> JoinHandle<anyhow::Result<()>> {
    std::thread::spawn(move || {
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

        loop {
            match kb_evt_rx.recv() {
                Ok(evt) => match evt {
                    keyboard::Event::Key(evt) => match evt.edge {
                        Edge::Rising => {
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
                        }
                        _ => {}
                    },
                },
                Err(_) => break,
            }
        }

        Ok(())
    })
}
