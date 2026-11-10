use crate::config::CustomUpload;
use crate::service::AnimationService;
use axum::extract::{Multipart, Path, Query, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use image::DynamicImage;
use serde::Deserialize;
use std::sync::Arc;
use tracing::{error, warn};

const DEFAULT_DELAY: u16 = 3;
const UPLOAD_FORM_NAME: &str = "image";

#[derive(Clone)]
pub struct AppState {
    pub service: AnimationService,
    pub custom_upload: CustomUpload,
}

#[derive(Debug, Deserialize, Default)]
pub struct AnimationQuery {
    pub delay: Option<String>,
    #[serde(rename = "no-cache")]
    pub no_cache: Option<String>,
}

pub fn parse_delay(value: Option<&str>) -> Result<u16, ()> {
    match value.map(str::trim) {
        None | Some("") => Ok(DEFAULT_DELAY),
        Some(v) => v.parse::<u16>().map_err(|_| ()).and_then(|d| {
            if d == 0 || d > 1000 {
                Err(())
            } else {
                Ok(d)
            }
        }),
    }
}

pub fn parse_discord_error(err: &anyhow::Error) -> &'static str {
    let msg = err.to_string();
    if msg.contains("10013") {
        "User not found"
    } else if msg.contains("50035") {
        "Incorrect user ID. Check ID for correctness"
    } else if msg.contains("get avatar error: not exists") {
        "User has no avatar"
    } else {
        "Something went wrong"
    }
}

fn no_cache_headers() -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("no-store, no-cache, must-revalidate, private"),
    );
    headers.insert(header::PRAGMA, HeaderValue::from_static("no-cache"));
    headers.insert(header::EXPIRES, HeaderValue::from_static("0"));
    headers
}

pub fn respond_content(
    content_type: &'static str,
    content: Vec<u8>,
    disable_cache: bool,
) -> Response {
    let mut headers = if disable_cache {
        no_cache_headers()
    } else {
        HeaderMap::new()
    };
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type));
    if let Ok(len_val) = HeaderValue::from_str(&content.len().to_string()) {
        headers.insert(header::CONTENT_LENGTH, len_val);
    }

    (StatusCode::OK, headers, content).into_response()
}

pub fn respond_soft_error(details: &str) -> Response {
    let body = format!(
        "<!DOCTYPE html><html><head><meta charset=\"UTF-8\"><title>PetPet - Error</title><meta content=\"Error\" property=\"og:title\"/><meta content=\"{}\" property=\"og:description\"/><meta content=\"https://github.com/UWblNSKPRIME/petpet-rs\" property=\"og:url\"/><meta content=\"#EE204D\" data-react-helmet=\"true\" name=\"theme-color\"/></head><body><p>{}</p></body></html>",
        details, details
    );
    respond_content("text/html", body.into_bytes(), true)
}

fn verify_discord_user_id(raw_param: &str) -> (String, Option<&'static str>) {
    let user_id = raw_param
        .strip_suffix(".webp")
        .or_else(|| raw_param.strip_suffix(".gif"))
        .unwrap_or(raw_param);

    if user_id.is_empty() {
        (user_id.to_string(), Some("No user ID was specified"))
    } else if user_id.eq_ignore_ascii_case("user_id") {
        (
            user_id.to_string(),
            Some("Replace `user_id` in the URL with real Discord user ID 😉"),
        )
    } else {
        (user_id.to_string(), None)
    }
}

pub async fn handle_discord(
    State(state): State<Arc<AppState>>,
    Path(raw_param): Path<String>,
    Query(query): Query<AnimationQuery>,
) -> Response {
    let (user_id, err_msg) = verify_discord_user_id(&raw_param);
    if let Some(msg) = err_msg {
        if user_id.is_empty() {
            warn!(user_id = %user_id, "Failed to get user ID");
        }
        return respond_soft_error(msg);
    }

    let Ok(delay) = parse_delay(query.delay.as_deref()) else {
        return respond_soft_error("Incorrect delay");
    };

    let disable_cache = query.no_cache.as_deref() == Some("true");

    match state.service.get_or_generate(&user_id, delay).await {
        Ok(data) => respond_content(state.service.mime_type(), data, disable_cache),
        Err(err) => {
            error!(user_id = %user_id, error = %err, "Error during WebP animation generation");
            respond_soft_error(parse_discord_error(&err))
        }
    }
}

pub async fn handle_custom(
    State(state): State<Arc<AppState>>,
    Query(query): Query<AnimationQuery>,
    multipart: Multipart,
) -> Response {
    let Ok(delay) = parse_delay(query.delay.as_deref()) else {
        return respond_soft_error("Incorrect delay");
    };

    let img = match decode_uploaded_image(multipart, &state.custom_upload).await {
        Ok(img) => img,
        Err(msg) => return respond_soft_error(&msg),
    };

    match state.service.generate_from_image(img, delay).await {
        Ok(data) => respond_content(state.service.mime_type(), data, true),
        Err(err) => {
            error!(error = %err, "Error during custom WebP animation generation");
            respond_soft_error("Failed to generate animation")
        }
    }
}

async fn decode_uploaded_image(
    mut multipart: Multipart,
    upload_cfg: &CustomUpload,
) -> Result<DynamicImage, String> {
    let mut found_bytes: Option<Vec<u8>> = None;

    while let Some(field) = multipart.next_field().await.map_err(|_| {
        format!(
            "Failed to parse upload. Make sure the file is smaller than {} bytes.",
            upload_cfg.max_upload_size
        )
    })? {
        if field.name() == Some(UPLOAD_FORM_NAME) {
            let data = field.bytes().await.map_err(|_| {
                format!(
                    "Failed to parse upload. Make sure the file is smaller than {} bytes.",
                    upload_cfg.max_upload_size
                )
            })?;
            if (data.len() as u64) > upload_cfg.max_upload_size {
                return Err(format!(
                    "Failed to parse upload. Make sure the file is smaller than {} bytes.",
                    upload_cfg.max_upload_size
                ));
            }
            found_bytes = Some(data.to_vec());
            break;
        }
    }

    let bytes = found_bytes.ok_or_else(|| "No image file was provided".to_string())?;

    let img = image::load_from_memory(&bytes).map_err(|_| {
        "Unsupported image format. Please upload a PNG, JPEG, or WebP.".to_string()
    })?;

    let width = img.width() as u64;
    let height = img.height() as u64;
    if width > 0 && height > 0 && width > upload_cfg.max_pixel_count / height {
        return Err(format!(
            "Image is too large. Maximum allowed size is {} pixels.",
            upload_cfg.max_pixel_count
        ));
    }

    Ok(img)
}
