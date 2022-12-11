use iced::widget::{button, column, text};
use iced::window::Mode;
use iced::{Alignment, Application, Command, Element, Settings};
use tokio_util::sync::CancellationToken;

pub struct Flags {
    rx: flume::Receiver<Message>,
    ct: CancellationToken,
}

pub fn run(flags: Flags) -> iced::Result {
    Counter::run(Settings {
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

struct Counter {
    rx: flume::Receiver<Message>,
    cancellation: CancellationToken,
}

#[derive(Debug, Clone, Copy)]
enum Message {
    IncrementPressed,
    DecrementPressed,
    Exit,
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
            _ => {}
        }

        Command::none()
    }

    fn view(&self) -> Element<Message> {
        column![
            button("Increment").on_press(Message::IncrementPressed),
            text("penis").size(50),
            button("Decrement").on_press(Message::DecrementPressed)
        ]
        .padding(20)
        .align_items(Alignment::Center)
        .into()
    }

    fn new(flags: Self::Flags) -> (Self, iced::Command<Self::Message>) {
        (
            Self {
                rx: flags.rx,
                cancellation: flags.ct,
            },
            iced::window::set_mode(Mode::Fullscreen),
        )
    }

    fn should_exit(&self) -> bool {
        self.cancellation.is_cancelled()
    }

    fn subscription(&self) -> iced::Subscription<Self::Message> {
        let rx = self.rx.clone();
        let ct = self.cancellation.clone();

        iced::subscription::unfold(0, (), |_| async move {
            let msg = tokio::select! {
                msg = rx.recv_async() => { match msg  {
                    Ok(msg) => msg,
                    Err(_) => Message::Exit,
                }}
                _ = ct.cancelled() => { Message::Exit }
            };

            (Some(msg), ())
        })
    }
}
