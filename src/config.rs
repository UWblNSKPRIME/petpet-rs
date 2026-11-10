use serde::Deserialize;
use std::env;
use std::fs;
use std::path::Path;
use std::time::Duration;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerHeartbeat {
    #[serde(default)]
    pub enable: bool,
    #[serde(default = "default_heartbeat_path")]
    pub path: String,
}

impl Default for ServerHeartbeat {
    fn default() -> Self {
        Self {
            enable: false,
            path: default_heartbeat_path(),
        }
    }
}

fn default_heartbeat_path() -> String {
    "/ping".to_string()
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerThrottle {
    #[serde(default)]
    pub enable: bool,
    #[serde(default = "default_throttle_limit")]
    pub limit: usize,
    #[serde(default = "default_throttle_backlog")]
    pub backlog: usize,
    #[serde(default = "default_throttle_backlog_timeout", with = "humantime_serde")]
    pub backlog_timeout: Duration,
}

impl Default for ServerThrottle {
    fn default() -> Self {
        Self {
            enable: false,
            limit: default_throttle_limit(),
            backlog: default_throttle_backlog(),
            backlog_timeout: default_throttle_backlog_timeout(),
        }
    }
}

fn default_throttle_limit() -> usize {
    2
}
fn default_throttle_backlog() -> usize {
    3
}
fn default_throttle_backlog_timeout() -> Duration {
    Duration::from_secs(5)
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerTLS {
    #[serde(default)]
    pub enable: bool,
    #[serde(default)]
    pub cert_file: String,
    #[serde(default)]
    pub key_file: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Server {
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default = "default_shutdown_timeout", with = "humantime_serde")]
    pub shutdown_timeout: Duration,
    #[serde(default = "default_write_timeout", with = "humantime_serde")]
    pub write_timeout: Duration,
    #[serde(default = "default_read_timeout", with = "humantime_serde")]
    pub read_timeout: Duration,
    #[serde(default = "default_idle_timeout", with = "humantime_serde")]
    pub idle_timeout: Duration,
    #[serde(default)]
    pub heartbeat: ServerHeartbeat,
    #[serde(default)]
    pub throttle: ServerThrottle,
    #[serde(default)]
    pub tls: ServerTLS,
}

impl Default for Server {
    fn default() -> Self {
        Self {
            host: default_host(),
            port: default_port(),
            shutdown_timeout: default_shutdown_timeout(),
            write_timeout: default_write_timeout(),
            read_timeout: default_read_timeout(),
            idle_timeout: default_idle_timeout(),
            heartbeat: ServerHeartbeat::default(),
            throttle: ServerThrottle::default(),
            tls: ServerTLS::default(),
        }
    }
}

fn default_host() -> String {
    "0.0.0.0".to_string()
}
fn default_port() -> u16 {
    3000
}
fn default_shutdown_timeout() -> Duration {
    Duration::from_secs(5)
}
fn default_write_timeout() -> Duration {
    Duration::from_secs(15)
}
fn default_read_timeout() -> Duration {
    Duration::from_secs(15)
}
fn default_idle_timeout() -> Duration {
    Duration::from_secs(60)
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Discord {
    #[serde(default = "default_true")]
    pub enable: bool,
    #[serde(default)]
    pub bot_token: String,
}

impl Default for Discord {
    fn default() -> Self {
        Self {
            enable: true,
            bot_token: String::new(),
        }
    }
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheMemoryConfig {
    #[serde(default = "default_memory_capacity")]
    pub capacity: usize,
}

impl Default for CacheMemoryConfig {
    fn default() -> Self {
        Self {
            capacity: default_memory_capacity(),
        }
    }
}

fn default_memory_capacity() -> usize {
    100
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheFSConfig {
    #[serde(default = "default_fs_path")]
    pub path: String,
    #[serde(default, with = "humantime_serde")]
    pub ttl: Duration,
}

impl Default for CacheFSConfig {
    fn default() -> Self {
        Self {
            path: default_fs_path(),
            ttl: Duration::ZERO,
        }
    }
}

fn default_fs_path() -> String {
    "./cache".to_string()
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheS3Config {
    #[serde(default)]
    pub bucket: String,
    #[serde(default)]
    pub endpoint: String,
    #[serde(default = "default_s3_region")]
    pub region: String,
    #[serde(default)]
    pub access_key: String,
    #[serde(default)]
    pub secret_key: String,
}

impl Default for CacheS3Config {
    fn default() -> Self {
        Self {
            bucket: String::new(),
            endpoint: String::new(),
            region: default_s3_region(),
            access_key: String::new(),
            secret_key: String::new(),
        }
    }
}

fn default_s3_region() -> String {
    "us-east-1".to_string()
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Cache {
    #[serde(default)]
    pub storage: String,
    #[serde(default)]
    pub memory: CacheMemoryConfig,
    #[serde(default)]
    pub fs: CacheFSConfig,
    #[serde(default)]
    pub s3: CacheS3Config,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Logger {
    #[serde(default = "default_logger_preset")]
    pub preset: String,
}

fn default_logger_preset() -> String {
    "prod".to_string()
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomUpload {
    #[serde(default = "default_true")]
    pub enable: bool,
    #[serde(default = "default_max_upload_size")]
    pub max_upload_size: u64,
    #[serde(default = "default_max_pixel_count")]
    pub max_pixel_count: u64,
}

impl Default for CustomUpload {
    fn default() -> Self {
        Self {
            enable: true,
            max_upload_size: default_max_upload_size(),
            max_pixel_count: default_max_pixel_count(),
        }
    }
}

fn default_max_upload_size() -> u64 {
    5_242_880
}
fn default_max_pixel_count() -> u64 {
    1_000_000
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    #[serde(default)]
    pub server: Server,
    #[serde(default)]
    pub discord: Discord,
    #[serde(default)]
    pub cache: Cache,
    #[serde(default)]
    pub logger: Logger,
    #[serde(default)]
    pub custom_upload: CustomUpload,
}

impl Config {
    pub fn load(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let mut cfg = if path.as_ref().exists() {
            let content = fs::read_to_string(path)?;
            serde_yaml::from_str::<Config>(&content)?
        } else {
            Config::default()
        };

        cfg.apply_env_overrides();
        Ok(cfg)
    }

    fn apply_env_overrides(&mut self) {
        if let Ok(v) = env::var("HOST") {
            self.server.host = v;
        }
        if let Ok(v) = env::var("PORT") {
            if let Ok(p) = v.parse() {
                self.server.port = p;
            }
        }
        if let Ok(v) = env::var("SHUTDOWN_TIMEOUT") {
            if let Ok(d) = humantime_serde::re::humantime::parse_duration(&v) {
                self.server.shutdown_timeout = d;
            }
        }
        if let Ok(v) = env::var("WRITE_TIMEOUT") {
            if let Ok(d) = humantime_serde::re::humantime::parse_duration(&v) {
                self.server.write_timeout = d;
            }
        }
        if let Ok(v) = env::var("READ_TIMEOUT") {
            if let Ok(d) = humantime_serde::re::humantime::parse_duration(&v) {
                self.server.read_timeout = d;
            }
        }
        if let Ok(v) = env::var("IDLE_TIMEOUT") {
            if let Ok(d) = humantime_serde::re::humantime::parse_duration(&v) {
                self.server.idle_timeout = d;
            }
        }
        if let Ok(v) = env::var("HEARTBEAT_ENABLE") {
            if let Ok(b) = v.parse() {
                self.server.heartbeat.enable = b;
            }
        }
        if let Ok(v) = env::var("HEARTBEAT_PATH") {
            self.server.heartbeat.path = v;
        }
        if let Ok(v) = env::var("THROTTLE_ENABLE") {
            if let Ok(b) = v.parse() {
                self.server.throttle.enable = b;
            }
        }
        if let Ok(v) = env::var("THROTTLE_LIMIT") {
            if let Ok(l) = v.parse() {
                self.server.throttle.limit = l;
            }
        }
        if let Ok(v) = env::var("THROTTLE_BACKLOG") {
            if let Ok(b) = v.parse() {
                self.server.throttle.backlog = b;
            }
        }
        if let Ok(v) = env::var("THROTTLE_BACKLOG_TIMEOUT") {
            if let Ok(d) = humantime_serde::re::humantime::parse_duration(&v) {
                self.server.throttle.backlog_timeout = d;
            }
        }
        if let Ok(v) = env::var("TLS_ENABLE") {
            if let Ok(b) = v.parse() {
                self.server.tls.enable = b;
            }
        }
        if let Ok(v) = env::var("TLS_CERT_FILE") {
            self.server.tls.cert_file = v;
        }
        if let Ok(v) = env::var("TLS_KEY_FILE") {
            self.server.tls.key_file = v;
        }
        if let Ok(v) = env::var("DISCORD_ENABLE") {
            if let Ok(b) = v.parse() {
                self.discord.enable = b;
            }
        }
        if let Ok(v) = env::var("BOT_TOKEN") {
            self.discord.bot_token = v;
        }
        if let Ok(v) = env::var("CACHE_STORAGE") {
            self.cache.storage = v;
        }
        if let Ok(v) = env::var("CACHE_MEMORY_CAPACITY") {
            if let Ok(c) = v.parse() {
                self.cache.memory.capacity = c;
            }
        }
        if let Ok(v) = env::var("CACHE_FS_PATH") {
            self.cache.fs.path = v;
        }
        if let Ok(v) = env::var("CACHE_FS_TTL") {
            if let Ok(d) = humantime_serde::re::humantime::parse_duration(&v) {
                self.cache.fs.ttl = d;
            }
        }
        if let Ok(v) = env::var("CACHE_S3_BUCKET") {
            self.cache.s3.bucket = v;
        }
        if let Ok(v) = env::var("CACHE_S3_ENDPOINT") {
            self.cache.s3.endpoint = v;
        }
        if let Ok(v) = env::var("CACHE_S3_REGION") {
            self.cache.s3.region = v;
        }
        if let Ok(v) = env::var("CACHE_S3_ACCESS_KEY") {
            self.cache.s3.access_key = v;
        }
        if let Ok(v) = env::var("CACHE_S3_SECRET_KEY") {
            self.cache.s3.secret_key = v;
        }
        if let Ok(v) = env::var("LOGGER_PRESET") {
            self.logger.preset = v;
        }
        if let Ok(v) = env::var("CUSTOM_UPLOAD_ENABLE") {
            if let Ok(b) = v.parse() {
                self.custom_upload.enable = b;
            }
        }
        if let Ok(v) = env::var("CUSTOM_MAX_UPLOAD_SIZE") {
            if let Ok(s) = v.parse() {
                self.custom_upload.max_upload_size = s;
            }
        }
        if let Ok(v) = env::var("CUSTOM_MAX_PIXEL_COUNT") {
            if let Ok(c) = v.parse() {
                self.custom_upload.max_pixel_count = c;
            }
        }
    }
}
