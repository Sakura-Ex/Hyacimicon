#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod account_storage;
mod accounts;
#[cfg(feature = "research")]
mod game_image_cache;
#[cfg(feature = "research")]
mod game_images;

use tauri::{Manager, PhysicalSize};

fn size_main_window(app: &tauri::App) -> tauri::Result<()> {
    let Some(window) = app.get_webview_window("main") else {
        return Ok(());
    };
    let Some(config) = app
        .config()
        .app
        .windows
        .iter()
        .find(|entry| entry.label == window.label())
    else {
        return Ok(());
    };
    let monitor = match window.current_monitor()? {
        Some(monitor) => Some(monitor),
        None => window.primary_monitor()?,
    };
    if let Some(monitor) = monitor {
        let screen = monitor.size();
        let scale = 0.75
            * (f64::from(screen.width) / config.width)
                .min(f64::from(screen.height) / config.height);
        window.set_size(PhysicalSize::new(
            (config.width * scale).round() as u32,
            (config.height * scale).round() as u32,
        ))?;
    }
    Ok(())
}

fn main() {
    let builder = tauri::Builder::default()
        .setup(|app| {
            if let Err(error) = size_main_window(app) {
                eprintln!("无法按屏幕设置窗口大小，使用默认尺寸：{error}");
            }
            Ok(())
        })
        .manage(accounts::AccountStore::default());
    #[cfg(feature = "research")]
    let builder = builder
        .register_uri_scheme_protocol("game-images", |_, request| game_images::serve(request))
        .manage(game_images::SyncLock::default())
        .invoke_handler(tauri::generate_handler![
            accounts::player_accounts,
            accounts::player_inspect_account,
            accounts::player_import_account,
            accounts::player_generate_account,
            accounts::player_account_catalogue,
            accounts::player_rename_account,
            accounts::player_delete_account,
            accounts::player_export_account,
            game_images::player_sync_game_images,
        ]);
    #[cfg(not(feature = "research"))]
    let builder = builder.invoke_handler(tauri::generate_handler![
        accounts::player_accounts,
        accounts::player_inspect_account,
        accounts::player_import_account,
        accounts::player_rename_account,
        accounts::player_delete_account,
        accounts::player_export_account,
    ]);
    builder
        .run(tauri::generate_context!())
        .expect("failed to run Hyacimicon");
}
