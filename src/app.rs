use egui::style::Margin;
use egui::{Align, Label, Layout, RichText, Sense, Vec2, Widget};

use std::collections::{BTreeSet, HashSet};
use std::ffi::{OsStr, OsString};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::spawn;
use tokio::sync::{watch, Mutex};
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, trace};

use crate::audio::{SoundId, SoundInfo};
use crate::driver::adafruit::seesaw::keypad;
use crate::driver::adafruit::seesaw::neopixel::Color;
use crate::{audio, keyboard};

struct App {
    state: Arc<Mutex<AppState>>,
    cancel: CancellationToken,
    kb_cmd_tx: flume::Sender<keyboard::Command>,
    audio_cmd_tx: flume::Sender<audio::Command>,
}

#[derive(Clone)]
enum AppState {
    Loading(LoadingState),
    Play(PlayState),
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

#[derive(Clone, Debug)]
struct PlayState {
    sounds: Vec<SoundInfo>,

    // 3 rows, 4 columns, b/c top row is reserved for fn keys
    sound_keys: [[SoundKeyState; 4]; 3],

    fn_keys: [FnKeyState; 4],

    reassign: Option<ReassignState>,

    quantize: bool,

    /// when a new sound is added to loops, this will control the period of that
    /// sound. None means looper is not active. Negative values mean it's a loop
    /// multiplier instead of a loop divider.
    loop_divider: Option<isize>,

    loops: Vec<LoopState>,

    beginning: Instant,

    /// how long is one tick? controls bpm
    tick: Duration,
}

impl PlayState {
    #[tracing::instrument(skip(self))]
    pub fn reassign_sound_begin(&mut self, key: (usize, usize)) -> &mut ReassignState {
        let base_dir = self
            .sounds
            .iter()
            .map(|s| &s.path)
            .fold(None, |acc, next| {
                Some(match acc {
                    Some(acc) => crate::util::path_intersection(acc, next),
                    None => next.to_owned(),
                })
            })
            .unwrap_or(PathBuf::new());

        let mut state = ReassignState {
            key,
            current_dir: base_dir.clone(),
            base_dir,
            sounds_in_dir: vec![],
            subdirs_in_dir: BTreeSet::new(),
            selection: None,
        };

        // update sounds_in_dir and subdirs_in_dir
        state.update(&self.sounds[..]);

        self.reassign = Some(state);

        self.reassign.as_mut().unwrap()
    }

    pub fn reassign_sound_save(&mut self) {
        if let Some(reassign) = &mut self.reassign {
            let (x, y) = reassign.key;
            self.sound_keys[y - 1][x].binding = reassign.selection;
            self.reassign_sound_quit();
        }
    }

    pub fn reassign_sound_quit(&mut self) {
        self.reassign = None;
    }

    pub fn reassign_sound_up(&mut self) {
        if let Some(reassign) = &mut self.reassign {
            reassign.up_dir(&self.sounds[..]);
        }
    }

    // current time of looper in ticks
    pub fn loop_time(&self) -> usize {
        let now = Instant::now();
        let time = now - self.beginning;
        (time.as_secs_f32() / self.tick.as_secs_f32()) as usize
    }

    pub fn add_to_loops(&mut self, sound: SoundId) {
        if let Some(loop_divider) = self.loop_divider {
            let period = if loop_divider < 0 {
                60 * -loop_divider
            } else if loop_divider == 0 {
                (self.sounds[sound.0].duration.as_secs_f32() / self.tick.as_secs_f32()) as isize
            } else {
                60 / loop_divider
            } as usize;

            let mut offset = self.loop_time();

            if self.quantize {
                offset = offset - (offset % period);
            }

            let ls = LoopState {
                offset: offset as isize,
                period,
                sound,
            };

            info!("adding sound to loops: {ls:?}");
            self.loops.push(ls);
        }
    }

    pub fn bpm_up(&mut self) {
        let bpm = f32::floor(1. / self.tick.as_secs_f32());
        self.tick = Duration::from_secs_f32(1. / (bpm + 1.5));
    }

    pub fn bpm_down(&mut self) {
        let bpm = f32::floor(1. / self.tick.as_secs_f32());
        self.tick = Duration::from_secs_f32(1. / (bpm - 0.5));
    }

    pub fn clear_loops(&mut self) {
        if let Some(_) = self.loop_divider {
            self.loops.clear();
            self.loop_divider = None;
        }
    }

    pub fn cycle_loop_mode(&mut self) {
        self.loop_divider = match self.loop_divider {
            None => Some(-8),
            Some(-8) => Some(-6),
            Some(-6) => Some(-4),
            Some(-4) => Some(-3),
            Some(-3) => Some(-2),
            Some(-2) => Some(0),
            // loop divider 0 means period is based on length of audio
            // useful for long snippets
            Some(0) => Some(1),
            // at 60 BPM, loop divider higher than 6 is probably not useful
            // fractional loop divider can only be factors of 60
            Some(1) => Some(2),
            Some(2) => Some(3),
            Some(3) => Some(4),
            Some(4) => Some(5),
            Some(5) => Some(6),
            Some(6) => None,
            // Some(10) => Some(12),
            // Some(12) => Some(15),
            // Some(15) => Some(20),
            // Some(20) => Some(30),
            // Some(30) => Some(60),
            // Some(60) => None,
            _ => unreachable!(),
        };
    }

    pub fn cycle_quantize(&mut self) {
        self.quantize = !self.quantize;
    }
}

#[derive(Clone, Debug)]
struct LoopState {
    /// offset from the start of the cycle in ticks
    offset: isize,
    /// period in ticks
    period: usize,
    sound: SoundId,
}

#[derive(Clone, Debug)]
struct ReassignState {
    key: (usize, usize),

    base_dir: PathBuf,
    current_dir: PathBuf,
    sounds_in_dir: Vec<SoundId>,
    subdirs_in_dir: BTreeSet<OsString>,

    selection: Option<SoundId>,
}

impl ReassignState {
    fn update(&mut self, sounds: &[SoundInfo]) {
        self.sounds_in_dir = sounds
            .iter()
            .filter_map(|s| {
                if let Some(parent) = s.path.parent() {
                    if parent == self.current_dir {
                        Some(s.id)
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect();

        self.sounds_in_dir.sort_by_key(|id| &sounds[id.0].path);

        self.subdirs_in_dir = sounds
            .iter()
            .filter_map(|s| {
                if let Ok(partial_dir) = s.path.strip_prefix(&self.current_dir) {
                    if partial_dir.iter().count() > 1 {
                        trace!(
                            "partial_dir = {partial_dir:?}, parent = {:?}, go",
                            partial_dir.parent()
                        );
                        // path has multiple segments, grab the first one
                        partial_dir.iter().nth(0)
                    } else {
                        trace!("partial_dir = {partial_dir:?}, no");
                        // this is the last segment of the path, meaning that this
                        // is not a subdir, but a file
                        None
                    }
                } else {
                    None
                }
            })
            .map(|s| s.to_owned())
            .collect();

        info!("subdirs = {:?}", &self.subdirs_in_dir);
    }

    #[tracing::instrument(skip(sounds))]
    pub fn select_dir(&mut self, dir: &OsStr, sounds: &[SoundInfo]) {
        info!("selecting dir");
        self.current_dir.push(dir);
        self.update(sounds);
    }

    #[tracing::instrument(skip(sounds))]
    pub fn up_dir(&mut self, sounds: &[SoundInfo]) {
        info!("going up a dir");
        if self.current_dir.starts_with(&self.base_dir) && self.current_dir != self.base_dir {
            self.current_dir.pop();
            self.update(sounds);
        }
    }

    #[tracing::instrument]
    pub fn select_sound(&mut self, id: SoundId) {
        info!("selecting sound");
        self.selection = Some(id);
    }
}

#[derive(Clone, Default, Debug)]
struct FnKeyState {
    pressed: bool,
}

#[derive(Clone, Default, Debug)]
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
        fullscreen: true,
        min_window_size: None,
        ..Default::default()
    };

    let state = Arc::new(Mutex::new(AppState::Loading(LoadingState {
        animation_cancel: loading_anim_ct,
        stage: LoadingStage::DiscoveringAudio,
    })));

    let (ctx_tx, ctx_rx) = watch::channel(None);

    spawn(process_loops(
        state.clone(),
        kb_cmd_tx.clone(),
        audio_cmd_tx.clone(),
    ));

    spawn(process_events(
        state.clone(),
        kb_cmd_tx.clone(),
        kb_evt_rx,
        audio_cmd_tx.clone(),
        audio_evt_rx,
        ctx_rx.clone(),
    ));

    spawn({
        let ct = ct.clone();
        async move {
            // request a repaint after cancellation so that the application called
            // eframe::App::update() and exits
            ct.cancelled().await;
            match &*ctx_rx.borrow() {
                Some(ctx) => ctx.request_repaint(),
                None => {}
            }
        }
    });

    eframe::run_native(
        "PI DJ",
        options,
        Box::new(move |cc| {
            cc.egui_ctx.set_pixels_per_point(4.);
            cc.egui_ctx.set_style(egui::Style {
                spacing: egui::style::Spacing {
                    window_margin: Margin::same(0.0),
                    item_spacing: Vec2::new(1.0, 1.0),
                    ..Default::default()
                },
                ..Default::default()
            });

            let _ = ctx_tx.send(Some(cc.egui_ctx.clone()));

            Box::new(App {
                state,
                cancel: ct,
                kb_cmd_tx,
                audio_cmd_tx,
            })
        }),
    );

    Ok(())
}

async fn process_loops(
    state: Arc<Mutex<AppState>>,
    kb_cmd_tx: flume::Sender<keyboard::Command>,
    audio_cmd_tx: flume::Sender<audio::Command>,
) {
    let mut interval = tokio::time::interval(Duration::from_millis(250));

    loop {
        let state = &*state.lock().await;
        match state {
            AppState::Play(state) if state.reassign.is_none() => {
                if interval.period() != state.tick {
                    interval = tokio::time::interval(state.tick)
                }

                let now = state.loop_time();

                // get loops that need to play on this tick
                let loops = state
                    .loops
                    .iter()
                    .filter(|l| (now as isize - l.offset).rem_euclid(l.period as isize) == 0);

                for l in loops {
                    let _ = audio_cmd_tx.send(audio::Command::Play { sound_id: l.sound });
                }

                if let Some(ld) = state.loop_divider {
                    if ld != 0 {
                        // blink loop divider LED (F4)
                        let ld_period = if ld > 0 { 60 / ld } else { 60 * -ld } as usize;

                        if now % ld_period == 0 {
                            set_solid_color(&kb_cmd_tx, 3, 0, Color::WHITE);
                        } else if now % ld_period == ld_period / 2 {
                            set_solid_color(&kb_cmd_tx, 3, 0, Color::BLACK);
                        }
                    }
                } else {
                    // clear the color
                    if now % 30 == 0 {
                        set_solid_color(&kb_cmd_tx, 3, 0, Color::BLACK);
                    }
                }
            }
            _ => {}
        };

        interval.tick().await;
    }
}

async fn process_events(
    state: Arc<Mutex<AppState>>,
    kb_cmd_tx: flume::Sender<keyboard::Command>,
    kb_evt_rx: flume::Receiver<keyboard::Event>,
    audio_cmd_tx: flume::Sender<audio::Command>,
    audio_evt_rx: flume::Receiver<audio::Event>,
    ctx_rx: watch::Receiver<Option<egui::Context>>,
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

        match &*ctx_rx.borrow() {
            Some(ctx) => ctx.request_repaint(),
            None => {}
        }
    }
}

async fn process_keyboard_event(
    state: &mut AppState,
    event: keyboard::Event,
    kb_cmd_tx: flume::Sender<keyboard::Command>,
    _kb_evt_rx: flume::Receiver<keyboard::Event>,
    audio_cmd_tx: flume::Sender<audio::Command>,
    _audio_evt_rx: flume::Receiver<audio::Event>,
) -> anyhow::Result<()> {
    match event {
        keyboard::Event::Key(key) => {
            let (x, y) = key.key;
            let (x, y) = (x as usize, y as usize);

            match state {
                AppState::Loading(_) => {}
                AppState::Play(state) => {
                    let pressed = match key.edge {
                        keypad::Edge::High | keypad::Edge::Rising => true,
                        keypad::Edge::Low | keypad::Edge::Falling => false,
                    };

                    if y == 0 {
                        state.fn_keys[x].pressed = pressed;
                    } else {
                        state.sound_keys[y - 1][x].pressed = pressed;
                    }

                    if state.reassign.is_some() {
                        if pressed {
                            if y == 0 {
                                match x {
                                    // F1 = exit
                                    0 => state.reassign_sound_quit(),
                                    // F2 = up one dir
                                    1 => state.reassign_sound_up(),
                                    // F3 = nothing
                                    2 => {}
                                    // F4 = select & exit
                                    3 => state.reassign_sound_save(),
                                    _ => unreachable!(),
                                }
                            }
                        }
                    } else {
                        if pressed {
                            if y > 0 {
                                if state.fn_keys[0].pressed {
                                    // F1 + button = reassign key
                                    state.reassign_sound_begin((x, y));
                                } else {
                                    // button = play sound if bound
                                    if let Some(id) = state.sound_keys[y - 1][x].binding {
                                        if state.loop_divider.is_some() {
                                            state.add_to_loops(id);
                                        }

                                        let _ = audio_cmd_tx
                                            .send(audio::Command::Play { sound_id: id });
                                    }
                                }
                            } else {
                                match x {
                                    // F1 = nothing
                                    0 => {}
                                    1 => {
                                        // F2 = toggle quantize
                                        state.cycle_quantize();
                                    }
                                    2 => {
                                        if state.fn_keys[0].pressed {
                                            // F0 + F3 = BPM down
                                            state.bpm_down();
                                        } else {
                                            // F3 = clear loops
                                            state.clear_loops();
                                        }
                                    }
                                    3 => {
                                        if state.fn_keys[0].pressed {
                                            // F0 + F4 = BPM up
                                            state.bpm_up();
                                        } else {
                                            // F4 = switch loop mode
                                            state.cycle_loop_mode();
                                        }
                                    }
                                    _ => unreachable!(),
                                }
                            }
                        }
                    }

                    update_keyboard_freeplay(state, kb_cmd_tx.clone());
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
    _kb_evt_rx: flume::Receiver<keyboard::Event>,
    _audio_cmd_tx: flume::Sender<audio::Command>,
    _audio_evt_rx: flume::Receiver<audio::Event>,
) -> anyhow::Result<()> {
    match event {
        audio::Event::LoadingEnd { sounds } => {
            if let AppState::Loading(state) = state {
                state.animation_cancel.cancel();
            }

            let inner = PlayState {
                sounds,
                sound_keys: Default::default(),
                fn_keys: Default::default(),
                reassign: None,
                loop_divider: None,
                quantize: true,
                beginning: Instant::now(),
                loops: vec![],
                tick: Duration::from_micros(1_000_000 / 60),
            };

            update_keyboard_freeplay(&inner, kb_cmd_tx.clone());
            *state = AppState::Play(inner);
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

        match state {
            AppState::Loading(_) => {
                egui::CentralPanel::default().show(ctx, |ui| {
                    ui.with_layout(
                        Layout::centered_and_justified(egui::Direction::TopDown)
                            .with_main_justify(false)
                            .with_cross_justify(false),
                        |ui| {
                            ui.group(|ui| {
                                Label::new("Loading").wrap(false).ui(ui);
                                ui.spinner();
                            });
                        },
                    )
                });
            }

            AppState::Play(state) => {
                egui::TopBottomPanel::bottom("bpm/div").show(ctx, |ui| {
                    ui.with_layout(Layout::left_to_right(Align::Max), |ui| {
                        ui.label(
                            RichText::new(match state.loop_divider {
                                Some(div) => {
                                    if div > 0 {
                                        format!("DIV = 1/{}", div)
                                    } else if div == 0 {
                                        format!("AUTODIV")
                                    } else {
                                        format!("DIV = {}", -div)
                                    }
                                }
                                None => format!("NODIV"),
                            })
                            .size(8.0),
                        );

                        ui.add_space(4.0);

                        let bpm = (1. / state.tick.as_secs_f32()) as usize;
                        ui.label(RichText::new(format!("BPM = {bpm}")).size(8.0));

                        if state.quantize {
                            ui.add_space(4.0);
                            ui.label(RichText::new(format!("Q")).size(8.0));
                        }
                    });
                });

                egui::CentralPanel::default().show(ctx, |ui| {
                    if state.reassign.is_some() {
                        render_reassign(ui, state, &self.kb_cmd_tx);
                        return;
                    }

                    egui::Grid::new("free_play").show(ui, |ui| {
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
                        ui.end_row();

                        for row in state.sound_keys.iter() {
                            for key in row.iter() {
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
                    });
                });
            }
        }

        // ctx.request_repaint();
    }
}

fn render_reassign(
    ui: &mut egui::Ui,
    state: &mut PlayState,
    kb_cmd_tx: &flume::Sender<keyboard::Command>,
) {
    let Some(reassign) = &mut state.reassign else { return; };
    let mut update_keyboard = false;

    ui.vertical(|ui| {
        let (x, y) = reassign.key;
        ui.label(format!("Reassigning key ({x}, {y})"));

        Label::new(egui::RichText::new(reassign.current_dir.to_string_lossy()).size(8.0))
            .wrap(false)
            .ui(ui);

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let mut selected_subdir = None;

                for subdir in &reassign.subdirs_in_dir {
                    let f = egui::containers::Frame::default()
                        .fill(egui::Color32::from_rgb(0, 0, 0))
                        .inner_margin(Margin::symmetric(3., 6.))
                        .show(ui, |ui| {
                            Label::new(RichText::new(subdir.to_string_lossy()).italics().size(8.))
                                .wrap(false)
                                .ui(ui);
                        });

                    if f.response.interact(Sense::click()).clicked() {
                        selected_subdir = Some(subdir.clone());
                    }
                }

                if let Some(selected_subdir) = selected_subdir {
                    reassign.select_dir(&selected_subdir, &state.sounds[..]);
                    update_keyboard = true;
                }

                let mut selected_sound = None;

                for id in &reassign.sounds_in_dir {
                    let sound_info = &state.sounds[id.0];

                    let f = egui::containers::Frame::default()
                        .fill(egui::Color32::from_rgb(0, 0, 0))
                        .inner_margin(Margin::symmetric(3., 6.))
                        .show(ui, |ui| {
                            let mut rt = RichText::new(
                                sound_info.path.file_name().unwrap().to_string_lossy(),
                            )
                            .size(8.);

                            if let Some(selection) = reassign.selection {
                                if selection == *id {
                                    rt = rt.strong();
                                }
                            }

                            Label::new(rt).wrap(false).ui(ui);
                        });

                    if f.response.interact(Sense::click()).clicked() {
                        selected_sound = Some(*id);
                    }
                }

                if let Some(selected_sound) = selected_sound {
                    reassign.select_sound(selected_sound);
                    update_keyboard = true;
                }
            });
    });

    if update_keyboard {
        update_keyboard_freeplay(state, kb_cmd_tx.clone());
    }
}

fn start_loading_animation(ct: CancellationToken, kb_cmd_tx: flume::Sender<keyboard::Command>) {
    std::thread::spawn(move || {
        debug!("initializing loading animation");

        for x in 0..4 {
            for y in 0..4 {
                set_solid_color(&kb_cmd_tx, x, y, Color::from_f32(0., 0., 0.3));
            }
        }

        let mut highlight = 15;

        while !ct.is_cancelled() {
            let x = highlight % 4;
            let y = highlight / 4;

            set_solid_color(&kb_cmd_tx, x, y, Color::from_f32(0., 0., 0.3));

            highlight = (highlight + 1) % 16;

            let x = highlight % 4;
            let y = highlight / 4;

            set_solid_color(&kb_cmd_tx, x, y, Color::from_f32(0., 0.2, 0.7));

            trace!("loading animation step");

            std::thread::sleep(Duration::from_millis(250));
        }

        debug!("exited loading animation");
    });
}

fn set_solid_color(kb_cmd_tx: &flume::Sender<keyboard::Command>, x: usize, y: usize, color: Color) {
    let _ = kb_cmd_tx.send(keyboard::Command::SetState {
        x: x as u16,
        y: y as u16,
        state: keyboard::PixelState::Solid {
            color,
            update: true,
        },
    });
}

fn update_keyboard_freeplay(state: &PlayState, kb_cmd_tx: flume::Sender<keyboard::Command>) {
    if let Some(reassign) = &state.reassign {
        set_solid_color(&kb_cmd_tx, 0, 0, Color::from_u8(255, 0, 0));
        set_solid_color(&kb_cmd_tx, 1, 0, Color::from_u8(255, 165, 0));
        set_solid_color(&kb_cmd_tx, 2, 0, Color::BLACK);

        // if something is selected, save button is bright green
        // otherwise, dim green
        if reassign.selection.is_some() {
            set_solid_color(&kb_cmd_tx, 3, 0, Color::from_u8(0, 255, 0));
        } else {
            set_solid_color(&kb_cmd_tx, 3, 0, Color::from_u8(0, 50, 0));
        }

        for x in 0..4 {
            for y in 1..4 {
                if (x, y) == reassign.key {
                    set_solid_color(&kb_cmd_tx, x, y, Color::WHITE);
                } else {
                    set_solid_color(&kb_cmd_tx, x, y, Color::BLACK);
                }
            }
        }

        return;
    }

    // F1 always white
    set_solid_color(&kb_cmd_tx, 0, 0, Color::WHITE);
    // F2 white if quantization is on
    set_solid_color(
        &kb_cmd_tx,
        1,
        0,
        if state.quantize {
            Color::WHITE
        } else {
            Color::BLACK
        },
    );
    // F3 always white
    set_solid_color(&kb_cmd_tx, 2, 0, Color::WHITE);
    // F4 controlled by the looper, don't touch

    for x in 0..4 {
        for y in 1..4 {
            let color = match state.sound_keys[y - 1][x].binding {
                Some(_) => Color::from_u8(50, 50, 50),
                None => Color::BLACK,
            };

            set_solid_color(&kb_cmd_tx, x, y, color);
        }
    }
}
