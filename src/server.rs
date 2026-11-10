use crate::cache::BytesCache;
use crate::config::Config;
use crate::discord::DiscordProvider;
use crate::handlers::{handle_custom, handle_discord, AppState};
use crate::service::AnimationService;
use anyhow::Result;
use axum::extract::DefaultBodyLimit;
use axum::routing::{get, post};
use axum::Router;
use std::sync::Arc;
use tower_http::trace::TraceLayer;

pub fn build_router(cfg: &Config, cache: Arc<dyn BytesCache>) -> Result<Router> {
    let provider = if cfg.discord.enable {
        Some(DiscordProvider::new(cfg.discord.bot_token.clone()))
    } else {
        None
    };

    let service = AnimationService::new(cache, provider);
    let state = Arc::new(AppState {
        service,
        custom_upload: cfg.custom_upload.clone(),
    });

    let mut router = Router::new().route(
        "/",
        get(|| async {
            "See documentation on GitHub: https://github.com/UWblNSKPRIME/petpet-rs"
        }),
    );

    if cfg.server.heartbeat.enable {
        router = router.route(&cfg.server.heartbeat.path, get(|| async { "." }));
    }

    if cfg.discord.enable {
        router = router
            .route("/discord/:user_id", get(handle_discord))
            .route("/ds/:user_id", get(handle_discord));
    }

    if cfg.custom_upload.enable {
        let max_upload = (cfg.custom_upload.max_upload_size as usize).saturating_add(65536);

        let custom_webp = post(handle_custom).layer(DefaultBodyLimit::max(max_upload));

        router = router
            .route("/custom/webp", custom_webp.clone())
            .route("/custom", custom_webp.clone())
            .route("/c/webp", custom_webp.clone())
            .route("/c", custom_webp);
    }

    router = router.layer(TraceLayer::new_for_http());

    Ok(router.with_state(state))
}
