mod app;
mod codex;
mod db;
mod tray;
mod window_event;
mod window_state;

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
            let app_handle = app.handle().clone();
            if let Err(error) = window_state::restore_main_window_size(&app_handle) {
                eprintln!("恢复主窗口尺寸失败: {error}");
            }

            if cfg!(debug_assertions) {
                let app_handle = app_handle.clone();
                tauri::async_runtime::block_on(async move {
                    app::log_database_startup_status(&app_handle).await;
                });
            }

            Ok(())
        })
        .on_window_event(window_event::handle_window_event)
        .invoke_handler(tauri::generate_handler![
            app::health_check,
            app::backup_database,
            app::restore_database,
            app::open_database_folder,
            app::get_codex_session_status,
            app::list_codex_sessions,
            app::prepare_codex_session_resume,
            app::get_codex_session_log_lines,
            app::get_task_latest_review,
            app::get_task_execution_change_history,
            app::start_task_code_review,
            app::read_image_file,
            app::open_task_attachment,
            app::create_project,
            app::update_project,
            app::delete_project,
            app::create_employee,
            app::update_employee,
            app::delete_employee,
            app::update_employee_status,
            app::create_task,
            app::add_task_attachments,
            app::update_task,
            app::update_task_status,
            app::delete_task,
            app::delete_task_attachment,
            app::create_subtask,
            app::update_subtask_status,
            app::delete_subtask,
            app::create_comment,
            codex::get_codex_settings,
            codex::update_codex_settings,
            codex::install_codex_sdk,
            codex::start_codex,
            codex::stop_codex,
            codex::restart_codex,
            codex::send_codex_input,
            codex::ai_suggest_assignee,
            codex::ai_analyze_complexity,
            codex::ai_generate_comment,
            codex::ai_generate_plan,
            codex::ai_split_subtasks,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
