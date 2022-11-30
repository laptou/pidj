use std::{
    fs::{self, File},
    io::BufReader,
    thread::JoinHandle,
    time::Duration,
};

use anyhow::Context;
use embedded_hal::prelude::_embedded_hal_blocking_delay_DelayUs;
use rodio::Decoder;
use rppal::i2c::I2c;
use tracing::info;

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

pub fn spawn_thread(
    cmd_rx: flume::Receiver<Command>,
    evt_tx: flume::Sender<Event>,
) -> JoinHandle<anyhow::Result<()>> {
    std::thread::spawn(move || {
        info!("locating audio files in cwd");

        let entries = fs::read_dir(std::env::current_dir().context("could not get cwd")?)
            .context("could not read cwd")?;

        let mut audio_file_paths = vec![];
        for entry in entries {
            let entry = entry.context("could not read cwd entry")?;
            if !entry.file_type()?.is_file() {
                continue;
            }

            let path = entry.path();

            match path.extension() {
                Some(ext) if ext == "mp3" => {
                    info!("located file {path:?}");
                    audio_file_paths.push(path);
                }
                _ => continue,
            }
        }

        let audio_files = audio_file_paths
            .into_iter()
            .map(|path| -> anyhow::Result<_> {
                let file = File::open(path).context("failed to open audio file")?;
                let reader = BufReader::new(file);
                let decoder = Decoder::new(reader).context("failed to decode audio file")?;
                Ok(decoder)
            })
            .collect::<Result<Vec<_>, _>>()
            .context("failed to load one or more audio files")?;

        info!("loaded {} audio files", audio_files.len());

        Ok(())
    })
}
