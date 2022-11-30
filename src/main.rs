use tokio::sync::{broadcast, mpsc};

mod driver;
mod keyboard;

fn main() -> anyhow::Result<()> {
    let (_cmd_tx, cmd_rx) = mpsc::channel(256);
    let (evt_tx, _evt_rx) = broadcast::channel(256);
    let kb_join = keyboard::spawn_keyboard_thread(cmd_rx, evt_tx);

    kb_join.join().unwrap()?;

    Ok(())
}
