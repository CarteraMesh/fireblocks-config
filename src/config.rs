#[cfg(feature = "gpg")]
use gpgme::{Context, Protocol};
#[cfg(feature = "xdg")]
use microxdg::XdgApp;
use {
    crate::{Error, OutputFormat, Result},
    config::{Config, File, FileFormat},
    serde::Deserialize,
    std::{
        fs,
        path::{Path, PathBuf},
        str::FromStr,
        time::Duration,
    },
};

fn expand_tilde(path: &str) -> PathBuf {
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

fn default_poll_timeout() -> Duration {
    Duration::from_secs(180)
}

fn default_poll_interval() -> Duration {
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
}

impl FireblocksConfig {
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
                tracing::warn!("profile config not found: {}", profile_config.display());
            }
        }

        Self::new(default_config, &profile_configs)
    }
}

#[cfg(test)]
mod tests {
    use {super::*, std::path::PathBuf};

    #[ignore]
    #[test]
    fn test_gpg_config() -> anyhow::Result<()> {
        let b = "examples/config-gpg.toml";
        let cfg = FireblocksConfig::new(b, &[])?;
        cfg.get_key()?;
        Ok(())
    }

    #[test]
    fn test_config() -> anyhow::Result<()> {
        let b = "examples/default.toml";
        let cfg = FireblocksConfig::new(b, &[])?;
        assert_eq!("blah", cfg.api_key);
        assert!(cfg.secret_path.is_some());
        if let Some(p) = cfg.secret_path.as_ref() {
            assert_eq!(PathBuf::from("examples/test.pem"), *p);
        }
        assert_eq!("https://sandbox-api.fireblocks.io/v1", cfg.url);
        assert_eq!(OutputFormat::Table, cfg.display_config.output);
        unsafe {
            std::env::set_var("FIREBLOCKS_SECRET", "override");
        }
        let cfg = FireblocksConfig::new(b, &[])?;
        assert!(cfg.secret.is_some());
        assert_eq!(String::from("override").as_bytes(), cfg.get_key()?);
        if let Some(ref k) = cfg.secret_path {
            assert_eq!(PathBuf::from("examples/test.pem"), *k);
        }

        assert_eq!(cfg.signer.vault, "0");
        unsafe {
            std::env::remove_var("FIREBLOCKS_SECRET");
        }
        Ok(())
    }

    #[test]
    fn test_config_override() -> anyhow::Result<()> {
        let b = "examples/default.toml";
        let cfg_override = "examples/override.toml";
        let cfg = FireblocksConfig::with_overrides(b, vec![cfg_override])?;
        assert_eq!("production", cfg.api_key);
        assert!(cfg.secret_path.is_some());
        if let Some(p) = cfg.secret_path.as_ref() {
            assert_eq!(PathBuf::from("examples/test.pem"), *p);
        }
        assert_eq!("https://api.fireblocks.io/v1", cfg.url);
        assert_eq!(OutputFormat::Table, cfg.display_config.output);
        Ok(())
    }

    #[test]
    fn test_embedded_key() -> anyhow::Result<()> {
        let b = "examples/default.toml";
        let cfg_override = "examples/embedded.toml";
        let cfg = FireblocksConfig::new(b, &[cfg_override])?;
        assert!(cfg.secret.is_some());
        let secret = cfg.secret.unwrap();
        assert_eq!(String::from("i am a secret").as_bytes(), secret.as_bytes());
        Ok(())
    }

    #[test]
    fn test_duration_parsing() -> anyhow::Result<()> {
        let b = "examples/default.toml";
        let cfg = FireblocksConfig::new(b, &[])?;

        // Verify that string values in TOML are parsed as Duration
        assert_eq!(cfg.signer.poll_timeout, Duration::from_secs(120));
        assert_eq!(cfg.signer.poll_interval, Duration::from_secs(5));

        Ok(())
    }

    #[cfg(feature = "xdg")]
    #[test]
    fn test_xdg_init() {
        // This test just ensures the XDG methods compile and can be called
        // In a real environment, it would try to load from ~/.config/fireblocks/
        match FireblocksConfig::init() {
            Ok(_) => {
                // Config loaded successfully from XDG directory
            }
            Err(_) => {
                // Expected if no config exists in XDG directory
                // This is fine for the compilation test
            }
        }

        // Test with &str slice
        match FireblocksConfig::init_with_profiles(&["test", "production"]) {
            Ok(_) => {
                // Config loaded successfully
            }
            Err(_) => {
                // Expected if no config exists
            }
        }

        // Test with Vec<String> to verify flexibility
        let profiles: Vec<String> = vec!["staging".to_string(), "production".to_string()];
        match FireblocksConfig::init_with_profiles(&profiles) {
            Ok(_) => {
                // Config loaded successfully
            }
            Err(_) => {
                // Expected if no config exists
            }
        }
    }
}
