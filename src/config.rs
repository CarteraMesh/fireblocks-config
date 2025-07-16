#[cfg(feature = "gpg")]
use gpgme::{Context, Protocol};
use {
    crate::{Error, OutputFormat, Result},
    config::{Config, File, FileFormat},
    serde::Deserialize,
    std::{
        fs,
        path::{Path, PathBuf},
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

#[derive(Clone, Debug, Default, Deserialize)]
pub struct Signer {
    pub poll_timeout: Option<u16>,
    pub poll_interval: Option<u16>,
    /// The vault id, default is "0"
    pub vault: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct FireblocksConfig {
    pub api_key: String,
    pub url: String,
    pub secret_path: Option<PathBuf>,
    pub secret_key: Option<String>,
    #[serde(rename = "display")]
    pub display_config: DisplayConfig,
    pub signer: Signer,
}

impl FireblocksConfig {
    pub fn get_key(&self) -> Result<Vec<u8>> {
        // Try secret_key first (simpler case)
        if let Some(ref key) = self.secret_key {
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
            .map_or(false, |ext| ext.eq_ignore_ascii_case("gpg"))
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
            std::env::set_var("FIREBLOCKS_SECRET_KEY", "override");
        }
        let cfg = FireblocksConfig::new(b, &[])?;
        assert!(cfg.secret_key.is_some());
        assert_eq!(String::from("override").as_bytes(), cfg.get_key()?);
        if let Some(ref k) = cfg.secret_path {
            assert_eq!(PathBuf::from("examples/test.pem"), *k);
        }

        assert_eq!(cfg.signer.vault, "0");
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
        assert!(cfg.secret_key.is_some());
        assert_eq!(String::from("i am a secret").as_bytes(), cfg.get_key()?);
        Ok(())
    }
}
