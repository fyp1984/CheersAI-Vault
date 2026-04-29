mod commands;
mod core;

use commands::{masking, crypto, sandbox, rules, batch, database, proxy, webview, gitea, file_manager, ocr, filebay_config, ai_model, platform};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // 设置 panic hook 来捕获崩溃信息
    std::panic::set_hook(Box::new(|panic_info| {
        eprintln!("=== PANIC OCCURRED ===");
        eprintln!("Panic info: {}", panic_info);
        
        if let Some(location) = panic_info.location() {
            eprintln!("Panic location: {}:{}:{}", location.file(), location.line(), location.column());
        }
        
        if let Some(payload) = panic_info.payload().downcast_ref::<&str>() {
            eprintln!("Panic payload: {}", payload);
        } else if let Some(payload) = panic_info.payload().downcast_ref::<String>() {
            eprintln!("Panic payload: {}", payload);
        }
        
        eprintln!("=== END PANIC INFO ===");
        
        // 尝试写入崩溃日志文件
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("crash.log") 
        {
            use std::io::Write;
            let _ = writeln!(file, "[{}] PANIC: {}", chrono::Utc::now(), panic_info);
        }
    }));

    println!("=== CheersAI Vault Starting ===");
    
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_shell::init())
        .manage(gitea::GiteaState::default())
        .manage(webview::BrowserFetchPending::default())
        .invoke_handler(tauri::generate_handler![
            masking::mask_file,
            masking::preview_masking,
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
