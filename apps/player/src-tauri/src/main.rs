#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod account_storage;
mod accounts;

fn main() {
    tauri::Builder::default()
        .manage(accounts::AccountStore::default())
        .invoke_handler(tauri::generate_handler![
            accounts::player_accounts,
            accounts::player_inspect_account,
            accounts::player_import_account,
            accounts::player_generate_account,
            accounts::player_account_catalogue,
            accounts::player_rename_account,
            accounts::player_delete_account,
            accounts::player_export_account,
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Hyacimicon");
}
