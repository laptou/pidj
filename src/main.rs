use tokio_util::sync::CancellationToken;
use tracing::{info, debug};
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

    let (app_cmd_tx, app_cmd_rx) = flume::unbounded();
    let (kb_cmd_tx, kb_cmd_rx) = flume::bounded(256);
    let (kb_evt_tx, kb_evt_rx) = flume::bounded(256);

    let kb_join = tokio::task::spawn_blocking({
        let ct = ct.clone();
        move || keyboard::run(ct, kb_cmd_rx, kb_evt_tx)
    });

    let audio_join = tokio::spawn(audio::run(ct.clone(), app_cmd_tx, kb_cmd_tx, kb_evt_rx));
    // let ct = ct.clone();
    // app::run(ct, app_cmd_rx).unwrap();
    debug!("hoho1");

    kb_join.await.unwrap()?;
    debug!("hoho2");
    audio_join.await.unwrap()?;
    debug!("hoho3");
    // app_join.join().unwrap()?;
    debug!("hoho4");

    info!("exit");

    Ok(())
}
