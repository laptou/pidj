use cancellation::CancellationTokenSource;
use tracing::info;
use tracing_subscriber::EnvFilter;

mod audio;
mod driver;
mod keyboard;

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .pretty()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let cts = CancellationTokenSource::new();
    let ct = cts.token().clone();

    ctrlc::set_handler(move || {
        info!("received ctrl+c, exiting");
        cts.cancel();
    })?;

    let (kb_cmd_tx, kb_cmd_rx) = flume::bounded(256);
    let (kb_evt_tx, kb_evt_rx) = flume::bounded(256);

    let kb_join = keyboard::spawn_thread(ct.clone(), kb_cmd_rx, kb_evt_tx);
    let audio_join = audio::spawn_thread(ct.clone(), kb_cmd_tx, kb_evt_rx);

    kb_join.join().unwrap()?;
    audio_join.join().unwrap()?;

    info!("exit");

    Ok(())
}
