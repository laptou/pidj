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

    let kb_join = tokio::task::spawn_blocking({
        let ct = ct.clone();
        move || keyboard::run(ct, kb_cmd_rx, kb_evt_tx)
    });

    let audio_join = tokio::spawn(audio::run(ct.clone(), audio_cmd_rx, audio_evt_tx));

    app::run(ct, kb_cmd_tx, kb_evt_rx, audio_cmd_tx, audio_evt_rx).unwrap();
    kb_join.await.unwrap()?;
    audio_join.await.unwrap()?;

    info!("exit");

    Ok(())
}
