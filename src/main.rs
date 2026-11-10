use anyhow::{anyhow, Result};
use axum_server::tls_rustls::RustlsConfig;
use petpet_rs::cache::create_cache;
use petpet_rs::config::Config;
use petpet_rs::server::build_router;
use std::net::SocketAddr;
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    let cfg = Config::load("config.yaml")?;

    init_logger(&cfg.logger.preset);

    let cache = create_cache(&cfg.cache)?;
    let router = build_router(&cfg, cache.clone())?;

    let addr_str = format!(
        "{}:{}",
        if cfg.server.host.is_empty() {
            "0.0.0.0"
        } else {
            &cfg.server.host
        },
        cfg.server.port
    );
    let addr: SocketAddr = addr_str
        .parse()
        .unwrap_or_else(|_| SocketAddr::from(([0, 0, 0, 0], cfg.server.port)));

    if cfg.server.tls.enable {
        if cfg.server.tls.cert_file.is_empty() || cfg.server.tls.key_file.is_empty() {
            return Err(anyhow!(
                "tls is enabled but certFile or keyFile is missing"
            ));
        }
        info!(address = %addr, "Starting the HTTPS server...");
        let tls_config =
            RustlsConfig::from_pem_file(&cfg.server.tls.cert_file, &cfg.server.tls.key_file)
                .await?;
        axum_server::bind_rustls(addr, tls_config)
            .serve(router.into_make_service())
            .await?;
    } else {
        info!(address = %addr, "Starting the HTTP server...");
        let listener = tokio::net::TcpListener::bind(addr).await?;
        axum::serve(listener, router)
            .with_graceful_shutdown(shutdown_signal())
            .await?;
    }

    info!("Closing the cache...");
    if let Err(e) = cache.close().await {
        error!(error = %e, "Error closing cache");
    }

    info!("Server exited properly");
    Ok(())
}

fn init_logger(preset: &str) {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    match preset {
        "gcp" | "prod" => {
            tracing_subscriber::fmt()
                .with_env_filter(filter)
                .json()
                .init();
        }
        _ => {
            tracing_subscriber::fmt().with_env_filter(filter).init();
        }
    }
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
    info!("Shutting down the server...");
}
