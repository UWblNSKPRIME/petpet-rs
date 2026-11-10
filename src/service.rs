use crate::cache::{BytesCache, CacheError};
use crate::discord::{DiscordProvider, UserAvatar};
use crate::petpet;
use anyhow::{anyhow, Result};
use image::DynamicImage;
use std::sync::Arc;
use tracing::{error, info};

#[derive(Clone)]
pub struct AnimationService {
    cache: Arc<dyn BytesCache>,
    provider: Option<DiscordProvider>,
}

impl AnimationService {
    pub fn new(cache: Arc<dyn BytesCache>, provider: Option<DiscordProvider>) -> Self {
        Self { cache, provider }
    }

    pub fn mime_type(&self) -> &'static str {
        "image/webp"
    }

    pub async fn get_or_generate(&self, user_id: &str, delay: u16) -> Result<Vec<u8>> {
        let provider = self
            .provider
            .as_ref()
            .ok_or_else(|| anyhow!("Discord provider is not configured"))?;

        let user_avatar = provider
            .get_user_avatar(user_id)
            .await
            .map_err(|e| anyhow!("error retrieving avatar: {}", e))?;

        let current_avatar_id = &user_avatar.id;
        let meta_key = format!("user-{}-avatar-id", user_id);
        let anim_cache_key = format!("user-{}-{}-webp", user_id, delay);
        let avatar_cache_key = format!("user-{}-avatar", user_id);

        // Check if the user's Discord avatar ID matches our cached avatar ID
        let cached_avatar_id = match self.cache.pull(&meta_key).await {
            Ok(bytes) => String::from_utf8(bytes).ok(),
            Err(_) => None,
        };

        let avatar_changed = cached_avatar_id.as_deref() != Some(current_avatar_id.as_str());

        if !avatar_changed {
            match self.cache.pull(&anim_cache_key).await {
                Ok(cached) => return Ok(cached),
                Err(CacheError::NotExists) => {}
                Err(e) => {
                    error!(user_id = %user_id, error = %e, "Error when pulling WebP from cache");
                }
            }
        } else if cached_avatar_id.is_some() {
            info!(
                user_id = %user_id,
                old_avatar = ?cached_avatar_id,
                new_avatar = %current_avatar_id,
                "Discord avatar updated, replacing old cached files"
            );
            let _ = self.cache.remove(&anim_cache_key).await;
            let _ = self.cache.remove(&avatar_cache_key).await;
        }

        let avatar_bytes = self
            .get_avatar_image(&user_avatar, &avatar_cache_key, avatar_changed)
            .await?;

        let decoded_image = image::load_from_memory(&avatar_bytes)
            .map_err(|e| anyhow!("error decoding avatar image: {}", e))?;

        let data = self.generate_from_image(decoded_image, delay).await?;

        // Overwrite the user's animation file and update stored avatar hash
        let cache_clone = self.cache.clone();
        let data_clone = data.clone();
        let user_id_owned = user_id.to_string();
        let current_avatar_owned = current_avatar_id.clone();
        tokio::spawn(async move {
            if let Err(e) = cache_clone.push(&anim_cache_key, &data_clone).await {
                error!(user_id = %user_id_owned, error = %e, "Error when pushing WebP to cache");
            }
            if let Err(e) = cache_clone
                .push(&meta_key, current_avatar_owned.as_bytes())
                .await
            {
                error!(user_id = %user_id_owned, error = %e, "Error when saving avatar ID to cache");
            }
        });

        Ok(data)
    }

    pub async fn generate_from_image(&self, img: DynamicImage, delay: u16) -> Result<Vec<u8>> {
        tokio::task::spawn_blocking(move || {
            let frames = petpet::make_animation(&img);
            let mut buf = Vec::with_capacity(48 * 1024);
            petpet::export_webp(&mut buf, &frames, delay)?;
            Ok(buf)
        })
        .await?
    }

    async fn get_avatar_image(
        &self,
        user_avatar: &UserAvatar,
        avatar_cache_key: &str,
        force_refresh: bool,
    ) -> Result<Vec<u8>> {
        if !force_refresh {
            match self.cache.pull(avatar_cache_key).await {
                Ok(cached) => return Ok(cached),
                Err(CacheError::NotExists) => {}
                Err(e) => {
                    error!(user_id = %user_avatar.user_id, error = %e, "Error when pulling avatar from cache");
                }
            }
        }

        let avatar_image = user_avatar
            .get_image()
            .await
            .map_err(|e| anyhow!("error retrieving avatar image: {}", e))?;

        let cache_clone = self.cache.clone();
        let img_clone = avatar_image.clone();
        let key_owned = avatar_cache_key.to_string();
        let user_id_owned = user_avatar.user_id.clone();
        tokio::spawn(async move {
            if let Err(e) = cache_clone.push(&key_owned, &img_clone).await {
                error!(user_id = %user_id_owned, error = %e, "Error when pushing avatar to cache");
            }
        });

        Ok(avatar_image)
    }
}
