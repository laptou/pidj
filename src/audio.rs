use std::{fs::File, future::Future, io::BufReader, path::PathBuf, sync::Arc, time::Duration};

use anyhow::Context;
use futures::stream::StreamExt;
use rodio::{Decoder, OutputStream, Source};
use tokio::{
    runtime::{self, Handle, Runtime},
    sync::{oneshot, Mutex},
    task::LocalSet,
};
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, trace, warn};

#[derive(Debug, Clone)]
pub enum Command {
    Play { sound_id: SoundId },
}

#[derive(Debug, Clone)]
pub enum Event {
    LoadingStart,
    LoadingEnd { sounds: Vec<SoundInfo> },
}

#[derive(Debug, Clone, PartialEq, PartialOrd, Eq, Ord, Hash, Copy)]
pub struct SoundId(pub usize);

#[derive(Debug, Clone)]
pub struct SoundInfo {
    pub id: SoundId,
    pub path: PathBuf,
    pub duration: Option<Duration>,
}

pub async fn run(
    ct: CancellationToken,
    cmd_rx: flume::Receiver<Command>,
    event_tx: flume::Sender<Event>,
) -> anyhow::Result<()> {
    let _ = event_tx.send(Event::LoadingStart);

    info!("locating audio files");

    let cwd = std::env::current_dir()?;
    let glob_pattern = cwd.to_string_lossy().to_string() + "/audio/**/*.{wav,flac,mp3}";

    debug!("globbing {glob_pattern:?}");

    let mut walkdir = async_walkdir::WalkDir::new(cwd.join("audio"));
    let mut paths = vec![];

    loop {
        tokio::select! {
            _ = ct.cancelled() => { break; }
            entry = walkdir.next() => {
                match entry {
                    Some(entry) => {
                        let entry = entry?;
                        let path = entry.path();

                        match path.extension() {
                            Some(ext) => {
                                match ext.to_str() {
                                    Some("wav") | Some("flac") | Some("mp3") => {
                                        trace!("loaded file {path:?}");
                                        paths.push(path.to_path_buf());
                                    }
                                    _ => {}
                                }
                            }
                            _ => {}
                        }
                    }
                    None => { break; }
                }
            }
        }
    }

    debug!("globbed");

    let (sounds, decoders): (Vec<_>, Vec<_>) = tokio::task::block_in_place(|| {
        paths
            .into_iter()
            .enumerate()
            .map(|(index, path)| -> anyhow::Result<_> {
                let file = File::open(&path).context("failed to open audio file")?;
                let reader = BufReader::new(file);
                let decoder = Decoder::new(reader)
                    .with_context(|| format!("failed to decode audio file {:?}", path))?;
                let decoder = decoder.convert_samples::<f32>().buffered();

                let sound = SoundInfo {
                    id: SoundId(index),
                    path,
                    duration: decoder.total_duration(),
                };

                Ok((sound, decoder))
            })
            .filter_map(|r| match r {
                Ok(r) => Some(r),
                Err(err) => {
                    warn!("failed to load sound: {err:?}");
                    None
                }
            })
            .unzip()
    });

    let _ = event_tx.send(Event::LoadingEnd { sounds });

    info!("loaded audio files");

    // rodio::OutputStream is !Send and !Sync, but if it is dropped, then the
    // rodio::OutputStreamHandle will stop working. This is the easiest way to
    // pin it to a single thread.

    let (tx, rx) = oneshot::channel();

    std::thread::spawn(move || {
        let rt = runtime::Builder::new_current_thread()
            .build()
            .expect("failed to construct tokio runtime");

        let result = rt.block_on(async {
            let (_stream, stream_handle) =
                OutputStream::try_default().context("no audio output stream available")?;

            debug!("opened audio output");

            loop {
                tokio::select! {
                    _ = ct.cancelled() => { break; }
                    cmd = cmd_rx.recv_async() => {
                        match cmd {
                            Ok(cmd) => match cmd {
                                Command::Play { sound_id } => {
                                    debug!("playing sound {sound_id:?}");

                                    stream_handle
                                        .play_raw(decoders[sound_id.0].clone())
                                        .context("failed to play sound")?;
                                }
                            },

                            Err(_) => break,
                        }
                    }
                }
            }

            Ok::<_, anyhow::Error>(())
        });

        let _ = tx.send(result);
    });

    rx.await??;

    debug!("exiting audio loop");

    Ok(())
}
