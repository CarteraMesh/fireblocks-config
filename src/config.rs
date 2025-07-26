#[cfg(feature = "gpg")]
use gpgme::{Context, Protocol};
#[cfg(feature = "xdg")]
use microxdg::XdgApp;
use {
    crate::{Error, OutputFormat, Result},
    config::{Config, File, FileFormat},
    serde::Deserialize,
    std::{
        collections::HashMap,
        fs,
        path::{Path, PathBuf},
        str::FromStr,
        time::Duration,
    },
};

pub(crate) fn expand_tilde(path: &str) -> PathBuf {
    if path.starts_with('~') {
        match dirs::home_dir() {
            Some(mut home) => {
                home.push(&path[2..]);
                home
            }
            None => PathBuf::from(path),
        }
    } else {
        PathBuf::from(path)
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct DisplayConfig {
    pub output: OutputFormat,
}

// Serde deserializer wrapper for parse_duration
fn deserialize_duration<'de, D>(deserializer: D) -> std::result::Result<Duration, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    let seconds = u64::from_str(&s)
        .map_err(|_| serde::de::Error::custom(format!("Invalid duration: {s}")))?;
    Ok(Duration::from_secs(seconds))
}

pub(crate) fn default_poll_timeout() -> Duration {
    Duration::from_secs(180)
}

pub(crate) fn default_poll_interval() -> Duration {
    Duration::from_secs(5)
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct Signer {
    #[serde(
        default = "default_poll_timeout",
        deserialize_with = "deserialize_duration"
    )]
    pub poll_timeout: Duration,
    #[serde(
        default = "default_poll_interval",
        deserialize_with = "deserialize_duration"
    )]
    pub poll_interval: Duration,
    /// The vault id
    pub vault: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct FireblocksConfig {
    pub api_key: String,
    pub url: String,
    pub secret_path: Option<PathBuf>,
    pub secret: Option<String>,
    #[serde(rename = "display")]
    pub display_config: DisplayConfig,
    pub signer: Signer,
    /// Arbitrary extra configuration values
    #[serde(default)]
    pub extra: HashMap<String, serde_json::Value>,
    /// Enable debug mode
    #[serde(default)]
    pub debug: bool,

    #[serde(default)]
    pub mainnet: bool,
}

impl FireblocksConfig {
    /// Get an extra configuration value as any deserializable type
    pub fn get_extra<T, K>(&self, key: K) -> Result<T>
    where
        T: serde::de::DeserializeOwned,
        K: AsRef<str>,
    {
        let key_str = key.as_ref();
        let value = self.extra.get(key_str).ok_or_else(|| Error::NotPresent {
            key: key_str.to_string(),
        })?;

        serde_json::from_value(value.clone()).map_err(|e| {
            Error::ConfigParseError(config::ConfigError::Message(format!(
                "Failed to deserialize key '{key_str}': {e}"
            )))
        })
    }

    /// Get an extra configuration value as a Duration from seconds
    ///
    /// This function retrieves a numeric value from the extra configuration
    /// and converts it to a `std::time::Duration` using
    /// `Duration::from_secs()`.
    ///
    /// # Arguments
    ///
    /// * `key` - The configuration key to look up (can be `&str`, `String`,
    ///   etc.)
    ///
    /// # Returns
    ///
    /// * `Ok(Duration)` - The duration value if the key exists and can be
    ///   parsed as u64
    /// * `Err(Error::NotPresent)` - If the key doesn't exist in the
    ///   configuration
    /// * `Err(Error::ConfigParseError)` - If the value cannot be deserialized
    ///   as u64
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use {fireblocks_config::FireblocksConfig, std::time::Duration};
    ///
    /// let config = FireblocksConfig::new("config.toml", &[])?;
    ///
    /// // Get timeout as Duration (assuming config has: timeout = 30)
    /// let timeout: Duration = config.get_extra_duration("timeout")?;
    /// assert_eq!(timeout, Duration::from_secs(30));
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn get_extra_duration<K>(&self, key: K) -> Result<Duration>
    where
        K: AsRef<str>,
    {
        let seconds: u64 = self.get_extra(key)?;
        Ok(Duration::from_secs(seconds))
    }

    /// Check if an extra configuration key exists
    pub fn has_extra<K>(&self, key: K) -> bool
    where
        K: AsRef<str>,
    {
        self.extra.contains_key(key.as_ref())
    }

    pub fn get_key(&self) -> Result<Vec<u8>> {
        // Try secret_key first (simpler case)
        if let Some(ref key) = self.secret {
            return Ok(key.clone().into_bytes());
        }

        // Then try secret_path
        let path = self.secret_path.as_ref().ok_or(Error::MissingSecret)?;
        let expanded_path = if path.starts_with("~") {
            expand_tilde(&path.to_string_lossy())
        } else {
            path.clone()
        };

        #[cfg(feature = "gpg")]
        if expanded_path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("gpg"))
        {
            return self.decrypt_gpg_file(&expanded_path);
        }

        // Regular file read
        fs::read(&expanded_path).map_err(|e| Error::IOError {
            source: e,
            path: expanded_path.to_string_lossy().to_string(),
        })
    }

    #[cfg(feature = "gpg")]
    fn decrypt_gpg_file(&self, path: &Path) -> Result<Vec<u8>> {
        let mut ctx = Context::from_protocol(Protocol::OpenPgp)?;

        let mut input = fs::File::open(path).map_err(|e| Error::IOError {
            source: e,
            path: path.to_string_lossy().to_string(),
        })?;

        let mut output = Vec::new();
        ctx.decrypt(&mut input, &mut output)?;

        Ok(output)
    }
}
impl FireblocksConfig {
    pub fn new<P: AsRef<Path>>(cfg: P, cfg_overrides: &[P]) -> Result<Self> {
        let cfg_path = cfg.as_ref();
        tracing::debug!("using config {}", cfg_path.display());

        let mut config_builder =
            Config::builder().add_source(File::new(&cfg_path.to_string_lossy(), FileFormat::Toml));

        // Add all override files in order
        for override_path in cfg_overrides {
            let path = override_path.as_ref();
            tracing::debug!("adding config override: {}", path.display());
            config_builder = config_builder
                .add_source(File::new(&path.to_string_lossy(), FileFormat::Toml).required(true));
        }

        // Environment variables still take highest precedence
        config_builder = config_builder
            .add_source(config::Environment::with_prefix("FIREBLOCKS").try_parsing(true));

        let conf: Self = config_builder.build()?.try_deserialize()?;
        tracing::trace!("loaded config {conf:#?}");
        Ok(conf)
    }

    pub fn with_overrides<P: AsRef<Path>>(
        cfg: P,
        overrides: impl IntoIterator<Item = P>,
    ) -> Result<Self> {
        let override_vec: Vec<P> = overrides.into_iter().collect();
        Self::new(cfg, &override_vec)
    }

    /// Load configuration from XDG config directory
    /// (~/.config/fireblocks/default.toml)
    #[cfg(feature = "xdg")]
    pub fn init() -> Result<Self> {
        Self::init_with_profiles::<&str>(&[])
    }

    /// Load configuration from XDG config directory with additional profile
    /// overrides
    ///
    /// Loads ~/.config/fireblocks/default.toml as base config, then applies
    /// each profile from ~/.config/fireblocks/{profile}.toml in order.
    ///
    /// # Example
    /// ```rust,no_run
    /// use fireblocks_config::FireblocksConfig;
    ///
    /// // Load default config only
    /// let config = FireblocksConfig::init()?;
    ///
    /// // Load default + production profile
    /// let config = FireblocksConfig::init_with_profiles(&["production"])?;
    ///
    /// // Load default + staging + production (layered)
    /// let config = FireblocksConfig::init_with_profiles(&["staging", "production"])?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[cfg(feature = "xdg")]
    pub fn init_with_profiles<S: AsRef<str>>(profiles: &[S]) -> Result<Self> {
        let xdg_app = XdgApp::new("fireblocks")?;
        let default_config = xdg_app.app_config_file("default.toml")?;

        tracing::debug!("loading default config: {}", default_config.display());

        let mut profile_configs = Vec::new();
        for profile in profiles {
            let profile_file = format!("{}.toml", profile.as_ref());
            let profile_config = xdg_app.app_config_file(&profile_file)?;
            if profile_config.exists() {
                tracing::debug!("adding profile config: {}", profile_config.display());
                profile_configs.push(profile_config);
            } else {
                return Err(Error::ProfileConfigNotFound(profile_file));
            }
        }

        Self::new(default_config, &profile_configs)
    }
}
