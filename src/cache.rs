use crate::config::{Cache as CacheConfig, CacheFSConfig, CacheS3Config};
use anyhow::{anyhow, Result};
use lru::LruCache;
use s3::bucket::Bucket;
use s3::creds::Credentials;
use s3::Region;
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};
use thiserror::Error;
use tokio::sync::Notify;

#[derive(Error, Debug)]
pub enum CacheError {
    #[error("cache item does not exist")]
    NotExists,
    #[error("cache error: {0}")]
    Other(#[from] anyhow::Error),
}

#[async_trait::async_trait]
pub trait BytesCache: Send + Sync {
    async fn push(&self, key: &str, value: &[u8]) -> Result<(), CacheError>;
    async fn pull(&self, key: &str) -> Result<Vec<u8>, CacheError>;
    async fn remove(&self, key: &str) -> Result<(), CacheError>;
    async fn close(&self) -> Result<(), CacheError>;
}

pub struct NoopCache;

#[async_trait::async_trait]
impl BytesCache for NoopCache {
    async fn push(&self, _key: &str, _value: &[u8]) -> Result<(), CacheError> {
        Ok(())
    }

    async fn pull(&self, _key: &str) -> Result<Vec<u8>, CacheError> {
        Err(CacheError::NotExists)
    }

    async fn remove(&self, _key: &str) -> Result<(), CacheError> {
        Ok(())
    }

    async fn close(&self) -> Result<(), CacheError> {
        Ok(())
    }
}

pub struct MemoryCache {
    lru: Mutex<LruCache<String, Vec<u8>>>,
}

impl MemoryCache {
    pub fn new(capacity: usize) -> Result<Self> {
        let cap = NonZeroUsize::new(capacity).ok_or_else(|| anyhow!("invalid cache capacity"))?;
        Ok(Self {
            lru: Mutex::new(LruCache::new(cap)),
        })
    }
}

#[async_trait::async_trait]
impl BytesCache for MemoryCache {
    async fn push(&self, key: &str, value: &[u8]) -> Result<(), CacheError> {
        let mut guard = self.lru.lock().map_err(|_| anyhow!("mutex poisoned"))?;
        guard.put(key.to_string(), value.to_vec());
        Ok(())
    }

    async fn pull(&self, key: &str) -> Result<Vec<u8>, CacheError> {
        let mut guard = self.lru.lock().map_err(|_| anyhow!("mutex poisoned"))?;
        match guard.get(key) {
            Some(v) => Ok(v.clone()),
            None => Err(CacheError::NotExists),
        }
    }

    async fn remove(&self, key: &str) -> Result<(), CacheError> {
        let mut guard = self.lru.lock().map_err(|_| anyhow!("mutex poisoned"))?;
        guard.pop(key);
        Ok(())
    }

    async fn close(&self) -> Result<(), CacheError> {
        Ok(())
    }
}

pub struct FileSystemCache {
    cache_dir: PathBuf,
    stop_notify: Arc<Notify>,
}

impl FileSystemCache {
    pub fn new(cfg: &CacheFSConfig) -> Result<Self> {
        let cache_dir = PathBuf::from(&cfg.path);
        if !cache_dir.exists() {
            std::fs::create_dir_all(&cache_dir)?;
        }

        let stop_notify = Arc::new(Notify::new());
        let ttl = cfg.ttl;
        if !ttl.is_zero() {
            let dir_clone = cache_dir.clone();
            let stop_clone = stop_notify.clone();
            let interval = if ttl > Duration::from_secs(60) {
                Duration::from_secs(60)
            } else {
                ttl
            };

            tokio::spawn(async move {
                let mut ticker = tokio::time::interval(interval);
                loop {
                    tokio::select! {
                        _ = ticker.tick() => {
                            Self::cleanup_expired(&dir_clone, ttl).await;
                        }
                        _ = stop_clone.notified() => {
                            break;
                        }
                    }
                }
            });
        }

        Ok(Self {
            cache_dir,
            stop_notify,
        })
    }

    async fn cleanup_expired(dir: &PathBuf, ttl: Duration) {
        let Ok(mut entries) = tokio::fs::read_dir(dir).await else {
            return;
        };
        let expire_before = SystemTime::now()
            .checked_sub(ttl)
            .unwrap_or(SystemTime::UNIX_EPOCH);
        while let Ok(Some(entry)) = entries.next_entry().await {
            let Ok(meta) = entry.metadata().await else {
                continue;
            };
            if !meta.is_file() {
                continue;
            }
            if let Ok(modified) = meta.modified() {
                if modified <= expire_before {
                    let _ = tokio::fs::remove_file(entry.path()).await;
                }
            }
        }
    }
}

#[async_trait::async_trait]
impl BytesCache for FileSystemCache {
    async fn push(&self, key: &str, value: &[u8]) -> Result<(), CacheError> {
        let path = self.cache_dir.join(key);
        tokio::fs::write(path, value)
            .await
            .map_err(|e| CacheError::Other(e.into()))
    }

    async fn pull(&self, key: &str) -> Result<Vec<u8>, CacheError> {
        let path = self.cache_dir.join(key);
        match tokio::fs::read(path).await {
            Ok(data) => Ok(data),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(CacheError::NotExists),
            Err(e) => Err(CacheError::Other(e.into())),
        }
    }

    async fn remove(&self, key: &str) -> Result<(), CacheError> {
        let path = self.cache_dir.join(key);
        match tokio::fs::remove_file(path).await {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(CacheError::Other(e.into())),
        }
    }

    async fn close(&self) -> Result<(), CacheError> {
        self.stop_notify.notify_waiters();
        Ok(())
    }
}

pub struct S3Cache {
    bucket: Box<Bucket>,
}

impl S3Cache {
    pub fn new(cfg: &CacheS3Config) -> Result<Self> {
        if cfg.bucket.is_empty() {
            return Err(anyhow!("bucket name is required"));
        }
        let region = if cfg.endpoint.is_empty() {
            cfg.region.parse().unwrap_or(Region::UsEast1)
        } else {
            Region::Custom {
                region: if cfg.region.is_empty() {
                    "us-east-1".to_string()
                } else {
                    cfg.region.clone()
                },
                endpoint: cfg.endpoint.clone(),
            }
        };

        let credentials = if !cfg.access_key.is_empty() || !cfg.secret_key.is_empty() {
            Credentials::new(
                Some(&cfg.access_key),
                Some(&cfg.secret_key),
                None,
                None,
                None,
            )?
        } else {
            Credentials::default()?
        };

        let mut bucket = Bucket::new(&cfg.bucket, region, credentials)?;
        if !cfg.endpoint.is_empty() {
            bucket.set_path_style();
        }

        Ok(Self { bucket })
    }
}

#[async_trait::async_trait]
impl BytesCache for S3Cache {
    async fn push(&self, key: &str, value: &[u8]) -> Result<(), CacheError> {
        self.bucket
            .put_object(key, value)
            .await
            .map_err(|e| CacheError::Other(e.into()))?;
        Ok(())
    }

    async fn pull(&self, key: &str) -> Result<Vec<u8>, CacheError> {
        match self.bucket.get_object(key).await {
            Ok(resp) if resp.status_code() == 200 => Ok(resp.to_vec()),
            Ok(resp) if resp.status_code() == 404 => Err(CacheError::NotExists),
            Ok(resp) => Err(CacheError::Other(anyhow!(
                "S3 status code: {}",
                resp.status_code()
            ))),
            Err(s3::error::S3Error::HttpFailWithBody(404, _)) => Err(CacheError::NotExists),
            Err(e) => Err(CacheError::Other(e.into())),
        }
    }

    async fn remove(&self, key: &str) -> Result<(), CacheError> {
        let _ = self.bucket.delete_object(key).await;
        Ok(())
    }

    async fn close(&self) -> Result<(), CacheError> {
        Ok(())
    }
}

pub fn create_cache(cfg: &CacheConfig) -> Result<Arc<dyn BytesCache>> {
    match cfg.storage.as_str() {
        "" | "none" | "noop" => Ok(Arc::new(NoopCache)),
        "memory" => Ok(Arc::new(MemoryCache::new(cfg.memory.capacity)?)),
        "fs" => Ok(Arc::new(FileSystemCache::new(&cfg.fs)?)),
        "s3" => Ok(Arc::new(S3Cache::new(&cfg.s3)?)),
        other => Err(anyhow!("unsupported cache storage {:?}", other)),
    }
}
