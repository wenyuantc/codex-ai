mod app;
mod codex;
mod db;
mod git_runtime;
mod git_workflow;
mod notifications;
mod process_spawn;
mod task_automation;
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
        .plugin(tauri_plugin_notification::init())
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

            task_automation::spawn_resume_pending_automation(app_handle.clone());

            Ok(())
        })
        .on_window_event(window_event::handle_window_event)
        .invoke_handler(tauri::generate_handler![
            app::health_check,
            app::backup_database,
            app::restore_database,
            app::open_database_folder,
            tray::show_main_window,
            app::get_employee_runtime_status,
            app::get_codex_session_status,
            app::sync_system_notifications,
            app::search_global,
            app::list_codex_sessions,
            app::prepare_codex_session_resume,
            app::get_codex_session_log_lines,
            app::get_task_latest_review,
            app::get_task_execution_change_history,
            app::get_codex_session_execution_change_history,
            app::get_codex_session_file_change_detail,
            git_workflow::get_project_git_overview,
            git_workflow::list_task_git_contexts,
            git_workflow::get_task_git_context,
            git_workflow::get_task_git_commit_overview,
            git_workflow::prepare_task_git_execution,
            git_workflow::refresh_task_git_context,
            git_workflow::reconcile_task_git_context,
            git_workflow::open_project_git_file,
            git_workflow::get_project_git_file_preview,
            git_workflow::stage_project_git_file,
            git_workflow::unstage_project_git_file,
            git_workflow::stage_all_project_git_files,
            git_workflow::unstage_all_project_git_files,
            git_workflow::stage_all_task_git_files,
            git_workflow::commit_project_git_changes,
            git_workflow::commit_task_git_changes,
            git_workflow::push_project_git_branch,
            git_workflow::pull_project_git_branch,
            git_workflow::delete_task_git_context_record,
            git_workflow::request_git_action,
            git_workflow::confirm_git_action,
            git_workflow::cancel_git_action,
            app::start_task_code_review,
            app::set_task_automation_mode,
            app::get_task_automation_state,
            task_automation::restart_task_automation,
            app::read_image_file,
            app::open_task_attachment,
            app::create_project,
            app::update_project,
            app::delete_project,
            app::list_ssh_configs,
            app::get_ssh_config,
            app::create_ssh_config,
            app::update_ssh_config,
            app::delete_ssh_config,
            app::probe_ssh_password_auth,
            app::validate_remote_codex_health,
            app::install_remote_codex_sdk,
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
            notifications::list_notifications,
            notifications::mark_notification_read,
            notifications::mark_all_notifications_read,
            codex::get_codex_settings,
            codex::get_remote_codex_settings,
            codex::update_codex_settings,
            codex::update_remote_codex_settings,
            codex::install_codex_sdk,
            codex::start_codex,
            codex::stop_codex_session,
            codex::stop_codex,
            codex::restart_codex,
            codex::send_codex_input,
            codex::ai_suggest_assignee,
            codex::ai_analyze_complexity,
            codex::ai_generate_comment,
            codex::ai_generate_commit_message,
            codex::ai_optimize_prompt,
            codex::ai_generate_plan,
            codex::ai_split_subtasks,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
