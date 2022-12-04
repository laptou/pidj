use std::sync::Arc;

use cancellation::CancellationToken;
use iced::widget::{button, column, text};
use iced::window::Mode;
use iced::{Alignment, Application, Command, Element, Sandbox, Settings};

pub struct Flags {
    ct: Arc<CancellationToken>,
}

pub fn run(flags: Flags) -> iced::Result {
    Counter::run(Settings {
        window: iced::window::Settings {
            always_on_top: true,
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

struct Counter {
    value: i32,
    cancellation: Arc<CancellationToken>,
}

#[derive(Debug, Clone, Copy)]
enum Message {
    IncrementPressed,
    DecrementPressed,
}

impl Application for Counter {
    type Message = Message;
    type Flags = Flags;

    type Executor = iced::executor::Default;
    type Theme = iced::Theme;

    fn title(&self) -> String {
        String::from("PI DJ")
    }

    fn update(&mut self, message: Message) -> Command<Message> {
        match message {
            Message::IncrementPressed => {
                self.value += 1;
            }
            Message::DecrementPressed => {
                self.value -= 1;
            }
        }

        Command::none()
    }

    fn view(&self) -> Element<Message> {
        column![
            button("Increment").on_press(Message::IncrementPressed),
            text(self.value).size(50),
            button("Decrement").on_press(Message::DecrementPressed)
        ]
        .padding(20)
        .align_items(Alignment::Center)
        .into()
    }

    fn new(flags: Self::Flags) -> (Self, iced::Command<Self::Message>) {
        (
            Self {
                value: 0,
                cancellation: flags.ct,
            },
            iced::window::set_mode(Mode::Fullscreen),
        )
    }
}
