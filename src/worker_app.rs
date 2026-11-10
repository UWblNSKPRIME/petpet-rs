use crate::petpet::{self, DEFAULT_DELAY};
use serde::Deserialize;
use std::collections::HashMap;
use worker::*;

const BASE_API_URL: &str = "https://discord.com/api/v10";
const BASE_CDN_URL: &str = "https://cdn.discordapp.com";
const HTTP_USER_AGENT: &str = "PetPet (https://github.com/UWblNSKPRIME/petpet-rs, 2.0) Cloudflare-Worker";

#[derive(Deserialize)]
struct DiscordUserResponse {
    avatar: Option<String>,
}

#[derive(Deserialize)]
struct JapiWrapper {
    data: Option<DiscordUserResponse>,
}

fn soft_error_response(details: &str) -> Result<Response> {
    let body = format!(
        "<!DOCTYPE html><html><head><meta charset=\"UTF-8\"><title>PetPet - Error</title><meta content=\"Error\" property=\"og:title\"/><meta content=\"{}\" property=\"og:description\"/><meta content=\"https://github.com/UWblNSKPRIME/petpet-rs\" property=\"og:url\"/><meta content=\"#EE204D\" data-react-helmet=\"true\" name=\"theme-color\"/></head><body><p>{}</p></body></html>",
        details, details
    );
    let headers = Headers::new();
    headers.set("Content-Type", "text/html; charset=utf-8")?;
    headers.set("Cache-Control", "no-store, no-cache, must-revalidate, private")?;
    headers.set("Pragma", "no-cache")?;
    headers.set("Expires", "0")?;
    Ok(Response::from_html(body)?.with_headers(headers))
}

fn webp_response(bytes: Vec<u8>, disable_cache: bool) -> Result<Response> {
    let headers = Headers::new();
    headers.set("Content-Type", "image/webp")?;
    headers.set("Content-Length", &bytes.len().to_string())?;
    if disable_cache {
        headers.set("Cache-Control", "no-store, no-cache, must-revalidate, private")?;
        headers.set("Pragma", "no-cache")?;
        headers.set("Expires", "0")?;
    } else {
        headers.set("Cache-Control", "public, max-age=3600")?;
    }
    Ok(Response::from_bytes(bytes)?.with_headers(headers))
}

fn parse_delay(value: Option<&str>) -> std::result::Result<u16, ()> {
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

async fn fetch_avatar_id(user_id: &str, bot_token: Option<String>) -> std::result::Result<String, String> {
    if let Some(token) = bot_token.filter(|t| !t.is_empty() && t != "your bot token") {
        let url = format!("{}/users/{}", BASE_API_URL, user_id);
        let headers = Headers::new();
        let _ = headers.set("Authorization", &format!("Bot {}", token));
        let _ = headers.set("User-Agent", HTTP_USER_AGENT);
        let _ = headers.set("Accept", "application/json");

        if let Ok(req) = Request::new_with_init(&url, RequestInit::new().with_method(Method::Get).with_headers(headers)) {
            if let Ok(mut resp) = Fetch::Request(req).send().await {
                if resp.status_code() == 200 {
                    if let Ok(text) = resp.text().await {
                        if let Ok(user) = serde_json::from_str::<DiscordUserResponse>(&text) {
                            return user.avatar.ok_or_else(|| "get avatar error: not exists".to_string());
                        }
                    }
                }
            }
        }
    }

    // Tokenless fallback via public Discord user lookup API (japi.rest)
    let fallback_url = format!("https://japi.rest/discord/v1/user/{}", user_id);
    let headers = Headers::new();
    let _ = headers.set("User-Agent", HTTP_USER_AGENT);
    let _ = headers.set("Accept", "application/json");

    let req = Request::new_with_init(
        &fallback_url,
        RequestInit::new().with_method(Method::Get).with_headers(headers),
    )
    .map_err(|e| e.to_string())?;

    let mut resp = Fetch::Request(req).send().await.map_err(|e| e.to_string())?;
    let body = resp.text().await.map_err(|e| e.to_string())?;

    if resp.status_code() != 200 {
        return Err(format!("error: {}", body));
    }

    let wrapper: JapiWrapper = serde_json::from_str(&body).map_err(|e| e.to_string())?;
    wrapper
        .data
        .and_then(|d| d.avatar)
        .ok_or_else(|| "get avatar error: not exists".to_string())
}

async fn fetch_avatar_bytes(user_id: &str, avatar_id: &str) -> std::result::Result<Vec<u8>, String> {
    // Download 256x256 lossless PNG from Discord CDN (2x supersampling for 128x128 output, fast on CF Workers)
    let url = format!("{}/avatars/{}/{}.png?size=256", BASE_CDN_URL, user_id, avatar_id);
    let headers = Headers::new();
    let _ = headers.set("User-Agent", HTTP_USER_AGENT);
    let _ = headers.set("Accept", "image/png");

    let req = Request::new_with_init(
        &url,
        RequestInit::new().with_method(Method::Get).with_headers(headers),
    )
    .map_err(|e| e.to_string())?;

    let mut resp = Fetch::Request(req).send().await.map_err(|e| e.to_string())?;
    if resp.status_code() != 200 && resp.status_code() != 304 {
        return Err(format!("invalid response status: {}", resp.status_code()));
    }

    resp.bytes().await.map_err(|e| e.to_string())
}

fn parse_discord_error(msg: &str) -> &'static str {
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

#[event(fetch)]
pub async fn main(mut req: Request, env: Env, ctx: Context) -> Result<Response> {
    let url = req.url()?;
    let path = url.path().to_string();
    let query_pairs: HashMap<String, String> = url.query_pairs().into_owned().collect();

    if path == "/" {
        return Response::ok("See documentation on GitHub: https://github.com/UWblNSKPRIME/petpet-rs");
    }

    if path == "/ping" {
        return Response::ok(".");
    }

    // Discord avatar endpoints: /ds/{user_id}, /ds/{user_id}.webp, /discord/{user_id}, /discord/{user_id}.webp
    if let Some(raw_param) = path
        .strip_prefix("/ds/")
        .or_else(|| path.strip_prefix("/discord/"))
    {
        let (user_id, err_msg) = verify_discord_user_id(raw_param);
        if let Some(msg) = err_msg {
            return soft_error_response(msg);
        }

        let Ok(delay) = parse_delay(query_pairs.get("delay").map(String::as_str)) else {
            return soft_error_response("Incorrect delay");
        };

        let disable_cache = query_pairs.get("no-cache").map(String::as_str) == Some("true");

        let bot_token = env.var("BOT_TOKEN").ok().map(|v| v.to_string());
        let avatar_id = match fetch_avatar_id(&user_id, bot_token).await {
            Ok(id) => id,
            Err(err) => return soft_error_response(parse_discord_error(&err)),
        };

        // Check Cloudflare Edge Cache and replace old cached animation if avatar_id changed
        let cache = Cache::default();
        let meta_key = format!("https://petpet-cache.internal/avatar-id/{}", user_id);
        let cache_key = format!("https://petpet-cache.internal/webp/{}/{}", user_id, delay);

        let mut avatar_changed = true;
        if let Ok(Some(mut meta_resp)) = cache.get(&meta_key, false).await {
            if let Ok(cached_id) = meta_resp.text().await {
                if cached_id.trim() == avatar_id {
                    avatar_changed = false;
                }
            }
        }

        if avatar_changed {
            let _ = cache.delete(&cache_key, false).await;
        } else if !disable_cache {
            if let Ok(Some(cached_resp)) = cache.get(&cache_key, false).await {
                return Ok(cached_resp);
            }
        }

        let avatar_bytes = match fetch_avatar_bytes(&user_id, &avatar_id).await {
            Ok(b) => b,
            Err(err) => return soft_error_response(parse_discord_error(&err)),
        };

        let decoded_img = match image::load_from_memory(&avatar_bytes) {
            Ok(img) => img,
            Err(_) => return soft_error_response("Something went wrong"),
        };

        let frames = petpet::make_animation(&decoded_img);
        let mut webp_buf = Vec::with_capacity(48 * 1024);
        if petpet::export_webp(&mut webp_buf, &frames, delay).is_err() {
            return soft_error_response("Something went wrong");
        }

        if !disable_cache {
            if let (Ok(resp_to_cache), Ok(meta_resp)) = (
                webp_response(webp_buf.clone(), false),
                Response::ok(avatar_id),
            ) {
                let meta_headers = Headers::new();
                let _ = meta_headers.set("Cache-Control", "public, max-age=86400");
                let meta_resp = meta_resp.with_headers(meta_headers);
                ctx.wait_until(async move {
                    let _ = cache.put(&cache_key, resp_to_cache).await;
                    let _ = cache.put(&meta_key, meta_resp).await;
                });
            }
        }

        return webp_response(webp_buf, disable_cache);
    }

    // Custom upload endpoints: POST /custom, /custom/webp, /c, /c/webp
    if req.method() == Method::Post
        && matches!(path.as_str(), "/custom" | "/custom/webp" | "/c" | "/c/webp")
    {
        let Ok(delay) = parse_delay(query_pairs.get("delay").map(String::as_str)) else {
            return soft_error_response("Incorrect delay");
        };

        let form = match req.form_data().await {
            Ok(f) => f,
            Err(_) => {
                return soft_error_response(
                    "Failed to parse upload. Make sure the file is smaller than 5242880 bytes.",
                )
            }
        };

        let Some(FormEntry::File(file)) = form.get("image") else {
            return soft_error_response("No image file was provided");
        };

        let bytes = match file.bytes().await {
            Ok(b) if b.len() <= 5_242_880 => b,
            _ => {
                return soft_error_response(
                    "Failed to parse upload. Make sure the file is smaller than 5242880 bytes.",
                )
            }
        };

        let Ok(img) = image::load_from_memory(&bytes) else {
            return soft_error_response(
                "Unsupported image format. Please upload a PNG, JPEG, or WebP.",
            );
        };

        if (img.width() as u64) * (img.height() as u64) > 1_000_000 {
            return soft_error_response(
                "Image is too large. Maximum allowed size is 1000000 pixels.",
            );
        }

        let frames = petpet::make_animation(&img);
        let mut webp_buf = Vec::with_capacity(48 * 1024);
        if petpet::export_webp(&mut webp_buf, &frames, delay).is_err() {
            return soft_error_response("Failed to generate animation");
        }

        return webp_response(webp_buf, true);
    }

    Response::error("Not Found", 404)
}
