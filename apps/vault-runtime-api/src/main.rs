use std::{net::{IpAddr, Ipv4Addr}, path::PathBuf};

use vault_runtime_api::{routes, Limits, Runtime};
use warp::Filter;

/// 等待 Ctrl-C（SIGINT）或——仅 Unix 平台——SIGTERM，任一到达即返回，
/// 供 `bind_with_graceful_shutdown` 触发停止监听。systemd 默认用 SIGTERM
/// 停止服务，因此 Linux 部署必须同时响应二者，不能只等 Ctrl-C。
async fn shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        let mut sigterm = signal(SignalKind::terminate())
            .expect("failed to install SIGTERM handler");
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                println!("vault-runtime-api received SIGINT, shutting down");
            }
            _ = sigterm.recv() => {
                println!("vault-runtime-api received SIGTERM, shutting down");
            }
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
        println!("vault-runtime-api received Ctrl-C, shutting down");
    }
}

#[tokio::main]
async fn main() {
    let data_root = std::env::var_os("VAULT_RUNTIME_DATA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("enterprise-data"));
    let port = std::env::var("VAULT_RUNTIME_PORT")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(8787);
    let bind_host = std::env::var("VAULT_RUNTIME_BIND_HOST")
        .ok()
        .and_then(|value| value.parse::<IpAddr>().ok())
        .unwrap_or(IpAddr::V4(Ipv4Addr::LOCALHOST));
    let origins = std::env::var("VAULT_RUNTIME_CORS_ORIGINS")
        .unwrap_or_else(|_| "http://127.0.0.1:5173,http://localhost:5173".to_string());
    let allowed_origins: Vec<String> = origins
        .split(',')
        .map(str::trim)
        .filter(|origin| !origin.is_empty() && *origin != "*")
        .map(str::to_string)
        .collect();
    if allowed_origins.is_empty() {
        eprintln!("Runtime startup failed: at least one explicit CORS origin is required");
        std::process::exit(2);
    }

    let runtime = match Runtime::initialize(data_root, Limits::default()).await {
        Ok(runtime) => runtime,
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(1);
        }
    };
    let cors = warp::cors()
        .allow_origins(allowed_origins.iter().map(String::as_str))
        .allow_headers(vec!["content-type"])
        .allow_methods(vec!["GET", "POST", "PUT", "DELETE"])
        .expose_header("X-Restored-Entity-Count");
    let api = routes(runtime).with(cors);
    println!("vault-runtime-api listening on http://{bind_host}:{port}");
    warp::serve(api)
        .bind_with_graceful_shutdown((bind_host, port), shutdown_signal())
        .1
        .await;
    println!("vault-runtime-api stopped");
}
