mod codex;
mod db;
mod tray;
mod window_event;

use std::sync::{Arc, Mutex};

use codex::CodexManager;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let migrations = db::migrations::get_all_migrations();

    tauri::Builder::default()
        .plugin(
            tauri_plugin_sql::Builder::default()
                .add_migrations("sqlite:codex-ai.db", migrations)
                .build(),
        )
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            tray::create_tray(app)?;
            app.manage(Arc::new(Mutex::new(CodexManager::new())));
            Ok(())
        })
        .on_window_event(window_event::handle_window_event)
        .invoke_handler(tauri::generate_handler![
            codex::start_codex,
            codex::stop_codex,
            codex::restart_codex,
            codex::send_codex_input,
            codex::ai_suggest_assignee,
            codex::ai_analyze_complexity,
            codex::ai_generate_comment,
            codex::ai_split_subtasks,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
