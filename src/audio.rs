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
use tracing::{debug, info, trace};

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
pub enum Command {}

#[derive(Debug, Clone, Copy)]
pub enum Event {}

pub fn spawn_thread(kb_evt_rx: flume::Receiver<keyboard::Event>) -> JoinHandle<anyhow::Result<()>> {
    std::thread::spawn(move || {
        info!("locating audio files");

        let cwd = std::env::current_dir()?;
        let glob_pattern = cwd.to_string_lossy() + "/**/*.{wav,flac,mp3}";

        debug!("globbing {glob_pattern:?}");

        let audio_files = globwalk::glob(glob_pattern)?
            .into_iter()
            .map(|entry| -> anyhow::Result<_> {
                let entry = entry?;
                let path = entry.path();
                let file = File::open(path).context("failed to open audio file")?;
                let reader = BufReader::new(file);
                let decoder = Decoder::new(reader).context("failed to decode audio file")?;
                trace!("loaded audio file {path:?}");
                Ok(decoder)
            })
            .collect::<Result<Vec<_>, _>>()
            .context("failed to load one or more audio files")?;

        info!("loaded {} audio files", audio_files.len());

        Ok(())
    })
}
