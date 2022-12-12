use embedded_hal::timer::Cancel;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::spawn;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;
use tracing::{debug, trace};

use crate::audio::{SoundId, SoundInfo};
use crate::driver::adafruit::seesaw::keypad;
use crate::driver::adafruit::seesaw::neopixel::Color;
use crate::{audio, keyboard};

struct App {
    state: Arc<Mutex<AppState>>,
    cancel: CancellationToken,
}

#[derive(Clone)]
enum AppState {
    Loading(LoadingState),
    FreePlay(FreePlayState),
}

#[derive(Clone)]
struct LoadingState {
    animation_cancel: CancellationToken,
    stage: LoadingStage,
}

#[derive(Clone)]
enum LoadingStage {
    DiscoveringAudio,
    BufferingAudio { progress: usize, num_files: usize },
}

#[derive(Clone)]
struct FreePlayState {
    sounds: Vec<SoundInfo>,

    // 3 rows, 4 columns, b/c top row is reserved for fn keys
    sound_keys: [[SoundKeyState; 4]; 3],

    fn_keys: [FnKeyState; 4],
}

#[derive(Clone, Default)]
struct FnKeyState {
    pressed: bool,
}

#[derive(Clone, Default)]
struct SoundKeyState {
    binding: Option<SoundId>,
    pressed: bool,
}

pub fn run(
    ct: tokio_util::sync::CancellationToken,
    kb_cmd_tx: flume::Sender<keyboard::Command>,
    kb_evt_rx: flume::Receiver<keyboard::Event>,
    audio_cmd_tx: flume::Sender<audio::Command>,
    audio_evt_rx: flume::Receiver<audio::Event>,
) -> Result<(), anyhow::Error> {
    let loading_anim_ct = ct.child_token();
    start_loading_animation(loading_anim_ct.clone(), kb_cmd_tx.clone());

    let options = eframe::NativeOptions {
        always_on_top: true,
        decorated: false,
        fullscreen: true,
        min_window_size: None,
        ..Default::default()
    };

    let state = Arc::new(Mutex::new(AppState::Loading(LoadingState {
        animation_cancel: loading_anim_ct,
        stage: LoadingStage::DiscoveringAudio,
    })));

    spawn(process_events(
        state.clone(),
        kb_cmd_tx,
        kb_evt_rx,
        audio_cmd_tx,
        audio_evt_rx,
    ));

    eframe::run_native(
        "PI DJ",
        options,
        Box::new(|cc| {
            cc.egui_ctx.set_pixels_per_point(4.);
            Box::new(App { state, cancel: ct })
        }),
    );

    Ok(())
}

async fn process_events(
    state: Arc<Mutex<AppState>>,
    kb_cmd_tx: flume::Sender<keyboard::Command>,
    kb_evt_rx: flume::Receiver<keyboard::Event>,
    audio_cmd_tx: flume::Sender<audio::Command>,
    audio_evt_rx: flume::Receiver<audio::Event>,
) -> anyhow::Result<()> {
    loop {
        tokio::select! {
            evt = kb_evt_rx.recv_async() => {
                let evt = evt?;
                process_keyboard_event(
                    &mut *state.lock().await,
                    evt,
                    kb_cmd_tx.clone(),
                    kb_evt_rx.clone(),
                    audio_cmd_tx.clone(),
                    audio_evt_rx.clone()
                ).await?;
            }
            evt = audio_evt_rx.recv_async() => {
                let evt = evt?;
                process_audio_event(
                    &mut *state.lock().await,
                    evt,
                    kb_cmd_tx.clone(),
                    kb_evt_rx.clone(),
                    audio_cmd_tx.clone(),
                    audio_evt_rx.clone()
                ).await?;
            }
        }
    }

    Ok(())
}

async fn process_keyboard_event(
    state: &mut AppState,
    event: keyboard::Event,
    kb_cmd_tx: flume::Sender<keyboard::Command>,
    kb_evt_rx: flume::Receiver<keyboard::Event>,
    audio_cmd_tx: flume::Sender<audio::Command>,
    audio_evt_rx: flume::Receiver<audio::Event>,
) -> anyhow::Result<()> {
    match event {
        keyboard::Event::Key(key) => {
            let (x, y) = key.key;
            let (x, y) = (x as usize, y as usize);
            let AppState::FreePlay(state) = state else { return Ok(()); };

            match key.edge {
                keypad::Edge::High | keypad::Edge::Rising => {
                    if y == 0 {
                        state.fn_keys[x].pressed = true;
                    }
                }
                keypad::Edge::Low | keypad::Edge::Falling => {
                    if y == 0 {
                        state.fn_keys[x].pressed = false;
                    }
                }
            }
        }
    }

    Ok(())
}

async fn process_audio_event(
    state: &mut AppState,
    event: audio::Event,
    kb_cmd_tx: flume::Sender<keyboard::Command>,
    kb_evt_rx: flume::Receiver<keyboard::Event>,
    audio_cmd_tx: flume::Sender<audio::Command>,
    audio_evt_rx: flume::Receiver<audio::Event>,
) -> anyhow::Result<()> {
    match event {
        audio::Event::LoadingEnd { sounds } => {
            if let AppState::Loading(state) = state {
                state.animation_cancel.cancel();
            }

            let inner = FreePlayState {
                sounds,
                sound_keys: Default::default(),
                fn_keys: Default::default(),
            };
            update_keyboard_freeplay(&inner, kb_cmd_tx.clone());
            *state = AppState::FreePlay(inner);
        }
        _ => {}
    }

    Ok(())
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        if self.cancel.is_cancelled() {
            debug!("cancelled, exiting app");
            frame.close();
            return;
        }

        let mut state = tokio::task::block_in_place(|| self.state.blocking_lock());
        let state = &mut *state;

        egui::CentralPanel::default().show(ctx, |ui| match state {
            AppState::Loading(_) => ui.centered_and_justified(|ui| {
                ui.group(|ui| {
                    ui.label("Loading");
                    ui.spinner();
                });
            }),
            AppState::FreePlay(state) => egui::Grid::new("free_play").show(ui, |ui| {
                for (i, fn_key) in state.fn_keys.iter().enumerate() {
                    ui.colored_label(
                        if fn_key.pressed {
                            egui::Color32::RED
                        } else {
                            egui::Color32::WHITE
                        },
                        format!("F{}", i),
                    );
                }

                for (i, row) in state.sound_keys.iter().enumerate() {
                    for (j, key) in row.iter().enumerate() {
                        ui.colored_label(
                            if key.pressed {
                                egui::Color32::RED
                            } else {
                                egui::Color32::WHITE
                            },
                            if key.binding.is_some() {
                                format!("X")
                            } else {
                                format!("?")
                            },
                        );
                    }
                    ui.end_row();
                }
            }),
        });

        ctx.request_repaint();
    }
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

            std::thread::sleep(Duration::from_millis(250));
        }

        debug!("exited loading animation");
    });
}

fn update_keyboard_freeplay(state: &FreePlayState, kb_cmd_tx: flume::Sender<keyboard::Command>) {
    for x in 0..4 {
        let _ = kb_cmd_tx.send(keyboard::Command::SetState {
            x,
            y: 0,
            state: keyboard::PixelState::Solid {
                color: Color::WHITE,
                update: true,
            },
        });
    }

    for x in 0..4 {
        for y in 1..4 {
            let key_state = match state.sound_keys[y - 1][x].binding {
                Some(_) => keyboard::PixelState::Solid {
                    color: Color {
                        r: 50,
                        g: 50,
                        b: 50,
                        w: 0,
                    },
                    update: true,
                },
                None => keyboard::PixelState::Solid {
                    color: Color::BLACK,
                    update: true,
                },
            };

            let _ = kb_cmd_tx.send(keyboard::Command::SetState {
                x: x as u16,
                y: y as u16,
                state: key_state,
            });
        }
    }
}

// fn on_audio_event(handle: &ExtEventSink, channels: &Channels, evt: audio::Event) {
//     let kb_cmd_tx = channels.kb_cmd_tx.clone();

//     match evt {
//         audio::Event::LoadingStart => {}
//         audio::Event::LoadingEnd { sounds } => {
//             let kb_cmd_tx = kb_cmd_tx.clone();
//             handle.add_idle_callback(move |data| {
//                 match data {
//                     AppState::Loading(LoadingState { animation_cancel }) => {
//                         animation_cancel.cancel()
//                     }
//                     _ => {
//                         panic!(
//                             "received audio::Event::LoadingEnd, but app was not in loading state"
//                         );
//                     }
//                 }

//                 let mut default_bindings: [[SoundKeyState; 4]; 3] = Default::default();

//                 for x in 0..4 {
//                     for y in 0..3 {
//                         default_bindings[y][x].binding = Some(SoundId(y * 4 + x));
//                     }
//                 }

//                 let inner = FreePlayState {
//                     sounds,
//                     sound_keys: default_bindings,
//                 };

//                 update_keyboard_freeplay(&inner, kb_cmd_tx.clone());

//                 *data = AppState::FreePlay(inner);
//             });
//         }
//     }
// }

// fn on_keyboard_event(handle: &ExtEventSink, channels: &Channels, evt: keyboard::Event) {
//     match evt {
//         keyboard::Event::Key(key) => {
//             handle.add_idle_callback({
//                 let audio_cmd_tx = channels.audio_cmd_tx.clone();
//                 move |data| match data {
//                     AppState::Loading(_) => {}
//                     AppState::FreePlay(data) => {
//                         let (x, y) = key.key;
//                         let x = x as usize;
//                         let y = y as usize;

//                         if y > 0 {
//                             match key.edge {
//                                 keypad::Edge::High | keypad::Edge::Rising => {
//                                     data.sound_keys[(y - 1)][x].pressed = true;

//                                     match data.sound_keys[(y - 1)][x].binding {
//                                         Some(id) => {
//                                             let _ = audio_cmd_tx
//                                                 .send(audio::Command::Play { sound_id: id });
//                                         }
//                                         _ => {}
//                                     }
//                                 }
//                                 keypad::Edge::Low | keypad::Edge::Falling => {
//                                     data.sound_keys[(y - 1)][x].pressed = false;
//                                 }
//                             }
//                         }
//                     }
//                 }
//             });
//         }
//     }
// }
