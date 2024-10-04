use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use futures::future::FutureExt;

use gossip_lib::GLOBALS;
use tokio::sync::{
    mpsc::{error::TryRecvError, UnboundedReceiver, UnboundedSender},
    RwLock,
};

mod facade;
mod notedata;
mod welcome;

slint::include_modules!();

pub fn run() -> Result<(), slint::PlatformError> {
    let (to_app, from_ui) = tokio::sync::mpsc::unbounded_channel();
    let (to_ui, from_app) = tokio::sync::mpsc::unbounded_channel();

    let app = App::new(to_app.clone(), to_ui);

    let ui = initialize_ui(app.clone())?;

    let worker = AppWorker::new(app, &ui, from_ui, to_app, from_app);

    if let Err(e) = ui.run() {
        tracing::error!("{}", e);
    }

    // shutdown
    if let Err(_) = worker.join() {
        tracing::error!("error on `worker.join()`");
    }

    // TODO save App state

    Ok(())
}

fn initialize_ui(app: Arc<RwLock<App>>) -> Result<AppWindow, slint::PlatformError> {
    let ui = AppWindow::new()?;

    facade::init(app.clone(), &ui)?;

    Ok(ui)
}

enum ToAppInstruction {
    Quit,
    PageChange(Option<AppPage>, Option<MainPage>),
    RequestRecompute,
}

enum ToUiInstruction {
    RequestPage(Option<AppPage>, Option<MainPage>),
}

pub struct App {
    app_page: Option<AppPage>,
    main_page: Option<MainPage>,
    last_feed_sync: Option<Instant>,
    to_app: UnboundedSender<ToAppInstruction>,
    to_ui: UnboundedSender<ToUiInstruction>,
}

impl App {
    fn new(
        to_app: UnboundedSender<ToAppInstruction>,
        to_ui: UnboundedSender<ToUiInstruction>,
    ) -> Arc<RwLock<Self>> {
        Arc::new(RwLock::new(Self {
            app_page: None,
            main_page: None,
            last_feed_sync: None,
            to_app,
            to_ui,
        }))
    }

    fn app_page_change(&mut self, page: AppPage) {
        match page {
            _ => {}
        }
    }

    fn main_page_change(&mut self, page: MainPage) {
        match page {
            MainPage::Feed => {
                GLOBALS.feed.switch_feed(gossip_lib::FeedKind::Inbox(false));
            }
            _ => {}
        }
    }
}

pub struct AppWorker {
    to_app: UnboundedSender<ToAppInstruction>,
    worker_thread: std::thread::JoinHandle<()>,
}

impl AppWorker {
    fn new(
        app: Arc<RwLock<App>>,
        ui: &AppWindow,
        from_ui: UnboundedReceiver<ToAppInstruction>,
        to_app: UnboundedSender<ToAppInstruction>,
        from_app: UnboundedReceiver<ToUiInstruction>,
    ) -> Self {
        let worker_thread = std::thread::spawn({
            let app = app.clone();
            let ui_weak = ui.as_weak();
            move || {
                tokio::runtime::Runtime::new()
                    .unwrap()
                    .block_on(app_worker_loop(app, ui_weak, from_ui, from_app))
                    .unwrap()
            }
        });
        Self {
            to_app,
            worker_thread,
        }
    }

    fn join(self) -> std::thread::Result<()> {
        let _ = self.to_app.send(ToAppInstruction::Quit);
        self.worker_thread.join()
    }
}

async fn app_worker_loop(
    app: Arc<RwLock<App>>,
    ui_weak: slint::Weak<AppWindow>,
    mut from_ui: UnboundedReceiver<ToAppInstruction>,
    mut from_app: UnboundedReceiver<ToUiInstruction>,
) -> tokio::io::Result<()> {
    loop {
        tokio::select! {
            res = facade::update_from_ui(app.clone(), ui_weak.clone()) => {
                res?;
            },
            res = facade::update_from_globals(app.clone(), ui_weak.clone()) => {
                res?;
            },
        };

        // slow down this worker
        tokio::time::sleep(Duration::from_millis(100)).await;

        match from_ui.try_recv() {
            Ok(msg) => match msg {
                ToAppInstruction::Quit => return Ok(()),
                ToAppInstruction::PageChange(maybe_app, maybe_main) => {
                    let mut rw_app = app.write().await;
                    if let Some(app_page) = maybe_app {
                        rw_app.app_page_change(app_page);
                    }
                    if let Some(main_page) = maybe_main {
                        rw_app.main_page_change(main_page);
                    }
                }
                ToAppInstruction::RequestRecompute => {
                    GLOBALS.feed.sync_recompute();
                }
            },
            Err(TryRecvError::Disconnected) => return Ok(()),
            Err(TryRecvError::Empty) => {}
        }

        match from_app.try_recv() {
            Ok(msg) => match msg {
                ToUiInstruction::RequestPage(app_page, main_page) => {
                    ui_weak
                        .clone()
                        .upgrade_in_event_loop(move |ui| {
                            let facade = ui.global::<Facade>();
                            if let Some(page) = app_page {
                                facade.set_app_page(page);
                            }
                            if let Some(page) = main_page {
                                facade.set_main_page(page);
                            }
                        })
                        .unwrap();
                }
            },
            Err(TryRecvError::Disconnected) => return Ok(()),
            Err(TryRecvError::Empty) => {}
        }
    }
}
