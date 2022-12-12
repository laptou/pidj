use std::path::PathBuf;
use std::time::Duration;

use druid::widget::{Align, Button, Flex, FlexParams, Label, ViewSwitcher};
use druid::{
    AppLauncher, Data, ExtEventSink, Lens, LocalizedString, PlatformError, Selector, Target,
    Widget, WidgetExt, WindowDesc,
};
use tokio_util::sync::CancellationToken;
use tracing::{debug, trace};

use crate::audio::{SoundId, SoundInfo};
use crate::driver::adafruit::seesaw::keypad;
use crate::driver::adafruit::seesaw::neopixel::Color;
use crate::{audio, keyboard};

#[derive(Data, Clone)]
enum AppData {
    Loading(LoadingState),
    FreePlay(FreePlayState),
}

#[derive(Clone)]
struct LoadingState {
    animation_cancel: CancellationToken,
}

impl Data for LoadingState {
    fn same(&self, _: &Self) -> bool {
        true
    }
}

#[derive(Data, Clone)]
struct FreePlayState {
    #[data(ignore)]
    sounds: Vec<SoundInfo>,

    // 3 rows, 4 columns, b/c top row is reserved for fn keys
    bindings: [[FreePlayBinding; 4]; 3],
}

#[derive(Data, Clone, Default)]
struct FreePlayBinding {
    #[data(eq)]
    binding: Option<SoundId>,
    pressed: bool,
}

struct AppDelegate;

impl druid::AppDelegate<AppData> for AppDelegate {
    fn event(
        &mut self,
        ctx: &mut druid::DelegateCtx,
        window_id: druid::WindowId,
        event: druid::Event,
        data: &mut AppData,
        env: &druid::Env,
    ) -> Option<druid::Event> {
        Some(event)
    }

    fn command(
        &mut self,
        ctx: &mut druid::DelegateCtx,
        target: Target,
        cmd: &druid::Command,
        data: &mut AppData,
        env: &druid::Env,
    ) -> druid::Handled {
        druid::Handled::No
    }

    fn window_added(
        &mut self,
        id: druid::WindowId,
        handle: druid::WindowHandle,
        data: &mut AppData,
        env: &druid::Env,
        ctx: &mut druid::DelegateCtx,
    ) {
    }

    fn window_removed(
        &mut self,
        id: druid::WindowId,
        data: &mut AppData,
        env: &druid::Env,
        ctx: &mut druid::DelegateCtx,
    ) {
    }
}

struct Channels {
    kb_cmd_tx: flume::Sender<keyboard::Command>,
    kb_evt_rx: flume::Receiver<keyboard::Event>,
    audio_cmd_tx: flume::Sender<audio::Command>,
    audio_evt_rx: flume::Receiver<audio::Event>,
}

pub fn run(
    ct: tokio_util::sync::CancellationToken,
    kb_cmd_tx: flume::Sender<keyboard::Command>,
    kb_evt_rx: flume::Receiver<keyboard::Event>,
    audio_cmd_tx: flume::Sender<audio::Command>,
    audio_evt_rx: flume::Receiver<audio::Event>,
) -> Result<(), PlatformError> {
    let loading_anim_ct = ct.child_token();
    start_loading_animation(loading_anim_ct.clone(), kb_cmd_tx.clone());

    let main_window = WindowDesc::new(ui_builder())
        .show_titlebar(false)
        .set_window_state(druid::WindowState::Maximized)
        .title("PIDJ");

    let launcher = AppLauncher::with_window(main_window)
        .delegate(AppDelegate)
        .configure_env(|env, _| {
            env.set(druid::theme::TEXT_SIZE_NORMAL, 30.0);
            env.set(druid::theme::TEXT_SIZE_LARGE, 40.0);
            env.set(druid::theme::BUTTON_BORDER_RADIUS, 0.0);
            env.set(druid::theme::BUTTON_BORDER_WIDTH, 0.0);
        });
    let handle = launcher.get_external_handle();

    tokio::spawn({
        let ct = ct.clone();
        async move {
            let channels = Channels {
                kb_cmd_tx,
                kb_evt_rx,
                audio_cmd_tx,
                audio_evt_rx,
            };

            loop {
                tokio::select! {
                    _ = ct.cancelled() => {
                        debug!("cancelled, closing all windows");
                        handle.submit_command(druid::commands::CLOSE_ALL_WINDOWS, (), Target::Auto).unwrap();
                        break;
                    }
                    msg = channels.audio_evt_rx.recv_async() => {
                        match msg {
                            Ok(evt) => on_audio_event(&handle, &channels, evt),
                            Err(_) => {
                                debug!("channel closed, closing all windows");
                                handle.submit_command(druid::commands::CLOSE_ALL_WINDOWS, (), Target::Auto).unwrap();
                                break;
                            },
                        }
                    }
                    msg = channels.kb_evt_rx.recv_async() => {
                        match msg {
                            Ok(evt) => on_keyboard_event(&handle, &channels, evt),
                            Err(_) => {
                                debug!("channel closed, closing all windows");
                                handle.submit_command(druid::commands::CLOSE_ALL_WINDOWS, (), Target::Auto).unwrap();
                                break;
                            },
                        }
                    }
                };
            }
        }
    });

    launcher.launch(AppData::Loading(LoadingState {
        animation_cancel: loading_anim_ct,
    }))
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
            let key_state = match state.bindings[y - 1][x].binding {
                Some(_) => keyboard::PixelState::Solid {
                    color: Color { r: 128, g: 128, b: 128, w: 0 },
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

fn on_audio_event(handle: &ExtEventSink, channels: &Channels, evt: audio::Event) {
    let kb_cmd_tx = channels.kb_cmd_tx.clone();

    match evt {
        audio::Event::LoadingStart => {}
        audio::Event::LoadingEnd { sounds } => {
            let kb_cmd_tx = kb_cmd_tx.clone();
            handle.add_idle_callback(move |data| {
                match data {
                    AppData::Loading(LoadingState { animation_cancel }) => {
                        animation_cancel.cancel()
                    }
                    other => {
                        panic!(
                            "received audio::Event::LoadingEnd, but app was not in loading state"
                        );
                    }
                }

                let mut default_bindings: [[FreePlayBinding; 4]; 3] = Default::default();

                for x in 0..4 {
                    for y in 0..3 {
                        default_bindings[y][x].binding = Some(SoundId(y * 4 + x));
                    }
                }

                let inner = FreePlayState {
                    sounds,
                    bindings: default_bindings,
                };

                update_keyboard_freeplay(&inner, kb_cmd_tx.clone());

                *data = AppData::FreePlay(inner);
            });
        }
    }
}

fn on_keyboard_event(handle: &ExtEventSink, channels: &Channels, evt: keyboard::Event) {
    let kb_cmd_tx = channels.kb_cmd_tx.clone();
    let audio_cmd_tx = channels.audio_cmd_tx.clone();

    match evt {
        keyboard::Event::Key(key) => {
            handle.add_idle_callback({
                let audio_cmd_tx = channels.audio_cmd_tx.clone();
                move |data| match data {
                    AppData::Loading(_) => {}
                    AppData::FreePlay(data) => {
                        let (x, y) = key.key;
                        let x = x as usize;
                        let y = y as usize;

                        if y > 0 {
                            match key.edge {
                                keypad::Edge::High | keypad::Edge::Rising => {
                                    data.bindings[(y - 1)][x].pressed = true;

                                    match data.bindings[(y - 1)][x].binding {
                                        Some(id) => {
                                            let _ = audio_cmd_tx.send(audio::Command::Play { sound_id: id });
                                        }
                                        _ => {}
                                    }
                                }
                                keypad::Edge::Low | keypad::Edge::Falling => {
                                    data.bindings[(y - 1)][x].pressed = false;
                                }
                            }
                        }
                    }
                }
            });
        }
    }
}

fn ui_builder() -> impl Widget<AppData> {
    let ui = ViewSwitcher::new(
        |data: &AppData, env| match data {
            AppData::Loading(_) => 0,
            AppData::FreePlay(_) => 1,
        },
        |_, data, env| match data {
            AppData::Loading(_) => Box::new(
                Align::centered(
                    Label::new("LOADING")
                        .with_text_size(30.0)
                        .with_text_alignment(druid::TextAlignment::Center)
                        .with_text_color(druid::Color::rgb8(255, 255, 255)),
                )
                .background(druid::Color::rgb8(0, 0, 255))
                .expand(),
            ),
            AppData::FreePlay(FreePlayState { sounds, bindings }) => {
                let mut grid = Flex::column();

                // fn row
                grid.add_flex_child(
                    {
                        let mut row = Flex::row();

                        for i in 0..4 {
                            let cell = Align::centered(
                                Label::new(format!("F{i}"))
                                    .with_text_size(30.0)
                                    .with_text_alignment(druid::TextAlignment::Center)
                                    .with_text_color(druid::Color::BLACK),
                            )
                            .background(druid::Color::WHITE);
                            row.add_flex_child(cell.expand(), 1.0);
                        }

                        row.expand()
                    },
                    1.0,
                );

                debug!("ree");

                // binding rows
                for i in 0..3 {
                    // fn row
                    grid.add_flex_child(
                        {
                            let mut row = Flex::row();

                            for j in 0..4 {
                                let binding = &bindings[i][j];

                                let text = match binding.binding {
                                    Some(binding) => {
                                        format!("S{}", binding.0)
                                    }
                                    None => {
                                        format!("??")
                                    }
                                };

                                let mut cell = Label::new(text)
                                    .with_text_size(30.0)
                                    .with_text_alignment(druid::TextAlignment::Center);

                                if binding.pressed {
                                    cell.set_text_color(druid::Color::rgb8(255, 0, 0));
                                }

                                row.add_flex_child(Align::centered(cell).expand(), 1.0);
                            }

                            row
                        },
                        1.0,
                    );
                }

                Box::new(grid.expand())
            }
        },
    );
    ui
}
