use gossip_lib::GLOBALS;
use notedata::NoteData;
use slint::{ModelRc, VecModel};

use super::*;

pub fn init(app: Arc<RwLock<App>>, ui: &AppWindow) -> Result<(), slint::PlatformError> {
    let facade = ui.global::<Facade>();

    facade.on_login(move |pwd| {
        if let Err(_) = gossip_lib::Overlord::unlock_key(pwd.to_string()) {
            todo!("Pass error messages back to user")
        }
    });

    facade.on_request_recompute(move || {
        app.clone()
            .blocking_read()
            .to_app
            .send(ToAppInstruction::RequestRecompute)
            .unwrap();
    });

    Ok(())
}

pub async fn update_from_ui(
    app: Arc<RwLock<App>>,
    ui_weak: slint::Weak<AppWindow>,
) -> std::io::Result<()> {
    {
        // ---- Page changes --------------------------------------------------
        let app = app.clone();
        ui_weak
            .clone()
            .upgrade_in_event_loop(move |ui| {
                let facade = ui.global::<Facade>();
                let ui_app_page = facade.get_app_page();
                let ui_main_page = facade.get_main_page();
                let mut wr_app = app.blocking_write();

                let new_app_page = if wr_app.app_page != Some(ui_app_page) {
                    Some(ui_app_page)
                } else {
                    None
                };

                let new_main_page = if wr_app.main_page != Some(ui_main_page) {
                    Some(ui_main_page)
                } else {
                    None
                };

                wr_app.app_page = Some(ui_app_page);
                wr_app.main_page = Some(ui_main_page);

                if new_app_page.is_some() || new_main_page.is_some() {
                    wr_app
                        .to_app
                        .send(ToAppInstruction::PageChange(new_app_page, new_main_page))
                        .unwrap();
                }
            })
            .unwrap();
    }

    Ok(())
}

pub async fn update_from_globals(
    app: Arc<RwLock<App>>,
    ui_weak: slint::Weak<AppWindow>,
) -> std::io::Result<()> {
    {
        // ---- UiSignerInfo --------------------------------------------------
        let (has_pubkey, pubkey_hex, pubkey_bech) = if let Some(pk) = GLOBALS.identity.public_key()
        {
            (true, pk.as_hex_string(), pk.as_bech32_string())
        } else {
            (false, "".to_owned(), "".to_owned())
        };

        let signer_info = UiSignerInfo {
            has_pubkey,
            has_signer: GLOBALS.identity.has_private_key(),
            is_unlocked: GLOBALS.identity.is_unlocked(),
            pubkey_bech: pubkey_bech.into(),
            pubkey_hex: pubkey_hex.into(),
        };

        ui_weak
            .clone()
            .upgrade_in_event_loop(move |ui| {
                let facade = ui.global::<Facade>();

                facade.set_signer(signer_info);
            })
            .unwrap();
    }
    {
        // ---- Feed ----------------------------------------------------------
        let last_feed_recompute = GLOBALS.feed.get_last_computed_time();
        let last_feed_sync = app.read().await.last_feed_sync;
        if last_feed_sync != last_feed_recompute {
            app.write().await.last_feed_sync = last_feed_recompute;
            ui_weak
                .clone()
                .upgrade_in_event_loop(move |ui| {
                    let mut notes: Vec<UiNoteData> = Vec::new();
                    // TODO: Create a note cache in App:: and only update changed events
                    for id in GLOBALS.feed.get_feed_events() {
                        if let Ok(Some(event)) = GLOBALS.db().read_event(id) {
                            notes.push(NoteData::fill_ui_notedata(event));
                        }
                    }
                    let feed_notes: ModelRc<UiNoteData> = ModelRc::new(VecModel::from(notes));
                    let facade = ui.global::<Facade>();
                    facade.set_feed_notes(feed_notes);
                })
                .unwrap();
        }
    }

    Ok(())
}
