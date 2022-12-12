use std::path::PathBuf;

use iced::widget::{button, column, text};
use iced::window::Mode;
use iced::{Alignment, Application, Command, Element, Settings};
use tokio_util::sync::CancellationToken;

pub struct Flags {
    pub rx: flume::Receiver<Message>,
    pub ct: CancellationToken,
}

pub fn run(flags: Flags) -> iced::Result {
    App::run(Settings {
        window: iced::window::Settings {
            always_on_top: true,
            decorations: false,
            ..Default::default()
        },
        flags,
        antialiasing: false,
        default_font: None,
        default_text_size: 20,
        exit_on_close_request: false,
        id: None,
        text_multithreading: true,
        try_opengles_first: false,
    })
}

#[derive(Debug)]
struct App {
    rx: flume::Receiver<Message>,
    cancellation: CancellationToken,
    sounds: Vec<Sound>,
    should_exit: bool,
}

#[derive(Debug, Clone)]
struct Sound {
    path: PathBuf,
}

#[derive(Debug, Clone)]
pub enum Message {
    NewSound { path: PathBuf },
    Exit,
}

impl Application for App {
    type Message = Message;
    type Flags = Flags;

    type Executor = iced::executor::Default;
    type Theme = iced::Theme;

    fn title(&self) -> String {
        String::from("PI DJ")
    }

    fn update(&mut self, message: Message) -> Command<Message> {
        match message {
            Message::NewSound { path } => {
                self.sounds.push(Sound { path });
            }
            Message::Exit => {
                self.should_exit = true;
            }
        }

        Command::none()
    }

    fn view(&self) -> Element<Message> {
        iced::widget::column(
            self.sounds
                .iter()
                .map(|s| iced::widget::text(s.path.to_string_lossy()).into())
                .collect(),
        )
        .padding(20)
        .into()
    }

    fn new(flags: Self::Flags) -> (Self, iced::Command<Self::Message>) {
        (
            Self {
                rx: flags.rx,
                cancellation: flags.ct,
                sounds: vec![],
                should_exit: false,
            },
            iced::Command::batch([
                iced::window::set_mode(Mode::Fullscreen),
                iced::Command::perform(async move {

                }, |_| ())
            ])
        )
    }

    fn should_exit(&self) -> bool {
        self.should_exit
    }

    fn subscription(&self) -> iced::Subscription<Self::Message> {
        let rx = self.rx.clone();
        let ct = self.cancellation.clone();

        iced::subscription::unfold(0, (ct, rx), |(ct, rx)| async move {
            let msg = tokio::select! {
                msg = rx.recv_async() => { 
                    match msg  {
                        Ok(msg) => msg,
                        Err(_) => Message::Exit,
                    }
                }
                _ = ct.cancelled() => { Message::Exit }
            };

            (Some(msg), (ct, rx))
        })
    }
}
