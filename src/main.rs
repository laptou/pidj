use tokio_util::sync::CancellationToken;
use tracing::{debug, info};
use tracing_subscriber::EnvFilter;

mod app;
mod audio;
mod driver;
mod keyboard;
mod util;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .pretty()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let ct = CancellationToken::new();

    ctrlc::set_handler({
        let ct = ct.clone();
        move || {
            info!("received ctrl+c, exiting");
            ct.cancel();
        }
    })?;

    let (kb_cmd_tx, kb_cmd_rx) = flume::bounded(256);
    let (kb_evt_tx, kb_evt_rx) = flume::bounded(256);

    let (audio_cmd_tx, audio_cmd_rx) = flume::bounded(256);
    let (audio_evt_tx, audio_evt_rx) = flume::bounded(256);

    let kb_join = std::thread::spawn({
        let ct = ct.clone();
        move || keyboard::run(ct, kb_cmd_rx, kb_evt_tx)
    });

    let async_join = std::thread::spawn({
        let ct = ct.clone();
        move || async_main(ct.clone(), audio_cmd_rx, audio_evt_tx)
    });

    app::run(ct.clone(), kb_cmd_tx, kb_evt_rx, audio_cmd_tx, audio_evt_rx)?;
    ct.cancel();

    async_join.join().unwrap()?;
    kb_join.join().unwrap()?;

    info!("exit");

    Ok(())
}

#[tokio::main]
async fn async_main(
    ct: CancellationToken,
    audio_cmd_rx: flume::Receiver<audio::Command>,
    audio_evt_tx: flume::Sender<audio::Event>,
) -> anyhow::Result<()> {
    let audio_join = tokio::spawn(audio::run(ct.clone(), audio_cmd_rx, audio_evt_tx));
    audio_join.await.unwrap()?;

    info!("async exit");

    Ok(())
}
