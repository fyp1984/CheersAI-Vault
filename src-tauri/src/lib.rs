mod commands;
mod core;

use commands::{masking, crypto, sandbox, rules, batch, database, proxy, webview, gitea, file_manager, ocr, filebay_config, vault_api_server, vault, ai_model, platform, installer, sensitive_terms, sync_config, extract_config};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // 设置 panic hook 来捕获崩溃信息到文件
    std::panic::set_hook(Box::new(|panic_info| {
        // 尝试写入崩溃日志文件
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("crash.log") 
        {
            use std::io::Write;
            let _ = writeln!(file, "PANIC: {}", panic_info);
            if let Some(location) = panic_info.location() {
                let _ = writeln!(file, "Location: {}:{}:{}", location.file(), location.line(), location.column());
            }
        }
    }));
    
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_shell::init())
        .manage(gitea::GiteaState::default())
        .manage(webview::BrowserFetchPending::default())
        .invoke_handler(tauri::generate_handler![
            masking::mask_file,
            masking::preview_masking,
            masking::save_preview_result,
            crypto::generate_passphrase,
            crypto::encrypt_mapping,
            crypto::decrypt_mapping,
            sandbox::has_pin,
            sandbox::verify_pin,
            sandbox::set_pin,
            sandbox::clear_pin,
            sandbox::list_sandbox_files,
            sandbox::list_files_in_directory,
            sandbox::export_sandbox,
            sandbox::import_sandbox,
            rules::get_rules,
            rules::save_rules,
            batch::start_batch_job,
            batch::get_batch_status,
            batch::cancel_batch_job,
            database::initialize_database,
            database::add_log_entry,
            database::get_logs,
            database::get_logs_count,
            database::clear_all_logs,
            database::cleanup_old_logs,
            database::save_user_setting,
            database::get_user_setting,
            database::get_all_user_settings,
            database::delete_user_setting,
            database::add_processing_history,
            database::get_processing_history,
            database::get_statistics,
            database::get_database_info,
            database::migrate_old_database,
            proxy::fetch_webpage,
            webview::open_webview_window,
            webview::open_desktop_window_with_button,
            webview::ensure_desktop_child_webview,
            webview::update_desktop_child_webview_bounds,
            webview::hide_desktop_child_webview,
            webview::navigate_webview,
            webview::webview_reload,
            webview::close_webview_window,
            webview::get_webview_url,
            webview::webview_eval_script,
            webview::navigate_main_window_with_button,
            webview::on_browser_fetch_result,
            webview::navigate_to_local,
            gitea::get_gitea_status,
            gitea::update_gitea_config,
            gitea::test_gitea_connection,
            gitea::create_gitea_repo,
            gitea::upload_to_gitea,
            gitea::upload_batch_to_gitea,
            gitea::delete_from_gitea,
            gitea::sync_filebay_config_from_desktop,
            gitea::get_filebay_token,
            file_manager::add_managed_file,
            file_manager::get_managed_files,
            file_manager::get_managed_file,
            file_manager::update_managed_file,
            file_manager::delete_managed_file,
            file_manager::delete_managed_files,
            file_manager::mark_file_uploaded,
            file_manager::search_managed_files,
            file_manager::get_file_statistics,
            commands::unmask::unmask_file,
            sandbox::delete_sandbox_file,
            sandbox::delete_sandbox_files,
            sandbox::get_sandbox_dir_path,
            sandbox::open_sandbox_dir,
            sandbox::clear_sandbox_dir,
            sandbox::lock_sandbox_files,
            sandbox::unlock_sandbox_files,
            ocr::check_ocr_installed,
            ocr::get_ocr_install_path,
            ocr::download_ocr_package,
            ocr::uninstall_ocr_package,
            filebay_config::read_filebay_config,
            filebay_config::check_filebay_config_exists,
            filebay_config::delete_filebay_config,
            filebay_config::validate_filebay_config_file,
            filebay_config::import_filebay_config,
            ai_model::check_ollama_installed,
            ai_model::check_ollama_binary_installed,
            ai_model::check_ollama_service_running,
            ai_model::download_ollama,
            ai_model::start_ollama_service,
            ai_model::check_ai_model_installed,
            ai_model::install_ai_model,
            ai_model::uninstall_ai_model,
            ai_model::call_ai_model,
            ai_model::get_ai_model_info,
            ai_model::check_ai_detection_available,
            platform::get_platform_context,
            sensitive_terms::add_sensitive_term,
            sensitive_terms::add_sensitive_terms_batch,
            sensitive_terms::update_sensitive_term,
            sensitive_terms::delete_sensitive_term,
            sensitive_terms::delete_sensitive_terms_batch,
            sensitive_terms::get_sensitive_terms,
            sensitive_terms::get_sensitive_term_categories,
            sensitive_terms::search_sensitive_terms,
            sensitive_terms::get_sensitive_terms_stats,
            sensitive_terms::export_sensitive_terms_csv,
            sensitive_terms::import_sensitive_terms_csv,
            installer::install_ocr_with_script,
            installer::uninstall_ocr_with_script,
            installer::install_ollama_with_script,
            installer::uninstall_ollama_with_script,
            installer::check_python_available,
            installer::get_ollama_installer_path,
            installer::open_installer_folder,
            vault_api_server::start_vault_api_server,
            vault_api_server::stop_vault_api_server,
            vault_api_server::check_vault_api_server_status,
            vault_api_server::save_filebay_config_via_api,
            vault_api_server::get_filebay_config_via_api,
            vault_api_server::delete_filebay_config_via_api,
            vault::list_vault_configs,
            vault::get_vault_config_by_user_id,
            vault::get_vault_config_by_email,
            vault::check_vault_db_exists,
            vault::get_vault_db_path_string,
            vault::get_vault_db_stats,
            sync_config::sync_config_from_desktop,
            extract_config::extract_config_from_desktop_webview,
            extract_config::eval_js_in_desktop_webview,
        ])
        .setup(|app| {
            // 暂时禁用 Vault API 服务器自动启动以排查崩溃问题
            // 用户可以通过界面手动启动
            let _ = app; // 避免未使用变量警告
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while running tauri application");
    
    app.run(|_app_handle, event| {
        if let tauri::RunEvent::ExitRequested { .. } = event {
            // 允许程序正常退出
        }
    });
}
