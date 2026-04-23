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
                    app::database::log_database_startup_status(&app_handle).await;
                });
            }

            task_automation::spawn_resume_pending_automation(app_handle.clone());

            Ok(())
        })
        .on_window_event(window_event::handle_window_event)
        .invoke_handler(tauri::generate_handler![
            app::database::health_check,
            app::database::backup_database,
            app::database::restore_database,
            app::database::open_database_folder,
            tray::show_main_window,
            app::employees::get_employee_runtime_status,
            app::employees::get_codex_session_status,
            app::remote::sync_system_notifications,
            app::sessions::search_global,
            app::sessions::list_codex_sessions,
            app::sessions::prepare_codex_session_resume,
            app::sessions::get_codex_session_log_lines,
            app::sessions::get_task_latest_review,
            app::sessions::get_task_execution_change_history,
            app::sessions::get_codex_session_execution_change_history,
            app::sessions::get_codex_session_file_change_detail,
            git_workflow::get_project_git_overview,
            git_workflow::list_project_git_commits,
            git_workflow::get_project_git_commit_detail,
            git_workflow::list_task_git_contexts,
            git_workflow::get_task_git_context,
            git_workflow::get_task_git_commit_overview,
            git_workflow::prepare_task_git_execution,
            git_workflow::refresh_task_git_context,
            git_workflow::reconcile_task_git_context,
            git_workflow::open_project_git_file,
            git_workflow::get_project_git_file_preview,
            git_workflow::get_project_git_commit_file_preview,
            git_workflow::stage_project_git_file,
            git_workflow::unstage_project_git_file,
            git_workflow::stage_all_project_git_files,
            git_workflow::unstage_all_project_git_files,
            git_workflow::rollback_project_git_files,
            git_workflow::rollback_all_project_git_changes,
            git_workflow::stage_all_task_git_files,
            git_workflow::commit_project_git_changes,
            git_workflow::commit_task_git_changes,
            git_workflow::push_project_git_branch,
            git_workflow::pull_project_git_branch,
            git_workflow::delete_task_git_context_record,
            git_workflow::request_git_action,
            git_workflow::confirm_git_action,
            git_workflow::cancel_git_action,
            app::review::start_task_code_review,
            app::tasks::set_task_automation_mode,
            app::tasks::get_task_automation_state,
            task_automation::restart_task_automation,
            app::review::read_image_file,
            app::review::open_task_attachment,
            app::projects::create_project,
            app::projects::update_project,
            app::projects::delete_project,
            app::remote::list_ssh_configs,
            app::remote::get_ssh_config,
            app::remote::create_ssh_config,
            app::remote::update_ssh_config,
            app::remote::delete_ssh_config,
            app::remote::probe_ssh_password_auth,
            app::remote::validate_remote_codex_health,
            app::remote::install_remote_codex_sdk,
            app::employees::create_employee,
            app::employees::update_employee,
            app::employees::delete_employee,
            app::employees::update_employee_status,
            app::tasks::create_task,
            app::tasks::add_task_attachments,
            app::tasks::update_task,
            app::tasks::update_task_status,
            app::tasks::delete_task,
            app::tasks::delete_task_attachment,
            app::tasks::create_subtask,
            app::tasks::update_subtask_status,
            app::tasks::delete_subtask,
            app::tasks::create_comment,
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
