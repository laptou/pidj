use std::path::PathBuf;

use druid::widget::{Button, Flex, Label};
use druid::{
    AppLauncher, Data, Lens, LocalizedString, PlatformError, Selector, Target, Widget, WidgetExt,
    WindowDesc,
};
use tracing::debug;

#[derive(Data, Clone, Lens)]
struct AppData {}

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

pub fn run(
    ct: tokio_util::sync::CancellationToken,
    rx: flume::Receiver<Message>,
) -> Result<(), PlatformError> {
    let main_window = WindowDesc::new(ui_builder)
        // .show_titlebar(false)
        .set_window_state(druid::WindowState::MAXIMIZED)
        .title("PIDJ");

    let launcher = AppLauncher::with_window(main_window).delegate(AppDelegate);
    let handle = launcher.get_external_handle();

    tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = ct.cancelled() => {
                    debug!("cancelled, closing all windows");
                    handle.submit_command(druid::commands::CLOSE_ALL_WINDOWS, (), Target::Auto).unwrap();
                    break;
                }
                msg = rx.recv_async() => {
                    match msg {
                        Ok(_) => {
                            debug!("hi");
                        },
                        Err(_) => {
                            debug!("channel closed, closing all windows");
                            handle.submit_command(druid::commands::CLOSE_ALL_WINDOWS, (), Target::Auto).unwrap();
                            break;
                        },
                    }
                }
            };
        }
    });

    launcher.launch(AppData {})
}

pub enum Message {
    NewSound { path: PathBuf },
}

fn ui_builder() -> impl Widget<AppData> {
    // The label text will be computed dynamically based on the current locale and count
    let text = LocalizedString::new("hello-counter")
        .with_arg("count", |data: &AppData, _env| "fack".into());
    let label = Label::new(text).with_text_size(50.0).padding(5.0).center();
    let button = Button::new("increment")
        .on_click(|_ctx, data, _env| {})
        .padding(5.0);

    Flex::column().with_child(label).with_child(button)
}
