use std::{net::Ipv4Addr, path::PathBuf};

use vault_runtime_api::{routes, Limits, Runtime};
use warp::Filter;

#[tokio::main]
async fn main() {
    let data_root = std::env::var_os("VAULT_RUNTIME_DATA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("enterprise-data"));
    let port = std::env::var("VAULT_RUNTIME_PORT")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(8787);
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
        .allow_methods(vec!["GET", "POST"])
        .expose_header("X-Restored-Entity-Count");
    let api = routes(runtime).with(cors);
    let address = (Ipv4Addr::LOCALHOST, port);
    println!("vault-runtime-api listening on http://127.0.0.1:{port}");
    warp::serve(api)
        .bind_with_graceful_shutdown(address, async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .1
        .await;
}
