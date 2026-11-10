use anyhow::{anyhow, Result};
use reqwest::Client;
use serde::Deserialize;

const BASE_API_URL: &str = "https://discord.com/api/v10";
const BASE_CDN_URL: &str = "https://cdn.discordapp.com";
const HTTP_USER_AGENT: &str =
    "PetPet (https://github.com/wavy-cat/petpet-go, 2.0) Rust/reqwest";

#[derive(Clone)]
pub struct DiscordProvider {
    token: String,
    client: Client,
}

pub struct UserAvatar {
    pub id: String,
    pub user_id: String,
    client: Client,
}

#[derive(Deserialize)]
struct DiscordUserResponse {
    avatar: Option<String>,
}

#[derive(Deserialize)]
struct JapiWrapper {
    data: Option<DiscordUserResponse>,
}

impl DiscordProvider {
    pub fn new(token: String) -> Self {
        let client = Client::builder()
            .user_agent(HTTP_USER_AGENT)
            .build()
            .expect("Failed to create reqwest client");
        Self { token, client }
    }

    pub async fn get_user_avatar(&self, user_id: &str) -> Result<UserAvatar> {
        let has_valid_bot_token =
            !self.token.is_empty() && self.token != "your bot token";

        if has_valid_bot_token {
            let url = format!("{}/users/{}", BASE_API_URL, user_id);
            let resp = self
                .client
                .get(&url)
                .header("Authorization", format!("Bot {}", self.token))
                .header("Accept", "application/json")
                .send()
                .await?;

            let status = resp.status();
            let body = resp.text().await?;

            if status.is_success() {
                let user: DiscordUserResponse = serde_json::from_str(&body)
                    .map_err(|e| anyhow!("error retrieving user: {}", e))?;

                let avatar_id = user
                    .avatar
                    .ok_or_else(|| anyhow!("get avatar error: not exists"))?;

                return Ok(UserAvatar {
                    id: avatar_id,
                    user_id: user_id.to_string(),
                    client: self.client.clone(),
                });
            }
        }

        // Tokenless fallback via public Discord user lookup API (japi.rest)
        let fallback_url = format!("https://japi.rest/discord/v1/user/{}", user_id);
        let resp = self
            .client
            .get(&fallback_url)
            .header("Accept", "application/json")
            .send()
            .await?;

        let status = resp.status();
        let body = resp.text().await?;

        if !status.is_success() {
            return Err(anyhow!("error: {}", body));
        }

        let wrapper: JapiWrapper = serde_json::from_str(&body)
            .map_err(|e| anyhow!("error retrieving user: {}", e))?;

        let avatar_id = wrapper
            .data
            .and_then(|d| d.avatar)
            .ok_or_else(|| anyhow!("get avatar error: not exists"))?;

        Ok(UserAvatar {
            id: avatar_id,
            user_id: user_id.to_string(),
            client: self.client.clone(),
        })
    }
}

impl UserAvatar {
    /// Downloads the Discord user's avatar in full quality (1024x1024 lossless PNG).
    pub async fn get_image(&self) -> Result<Vec<u8>> {
        let url = format!(
            "{}/avatars/{}/{}.png?size=1024",
            BASE_CDN_URL, self.user_id, self.id
        );
        let resp = self
            .client
            .get(&url)
            .header("Accept", "image/png")
            .send()
            .await?;

        let status = resp.status();
        if !status.is_success() && status.as_u16() != 304 {
            return Err(anyhow!("invalid response status: {}", status));
        }

        let bytes = resp.bytes().await?;
        Ok(bytes.to_vec())
    }
}
