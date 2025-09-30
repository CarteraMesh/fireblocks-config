#![doc = include_str!("../README.md")]
mod config;
mod error;
pub use error::Error;
use serde::Deserialize;
pub type Result<T> = std::result::Result<T, error::Error>;
use clap::ValueEnum;
pub use config::*;

#[derive(Copy, Deserialize, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum, Debug, Default)]
pub enum OutputFormat {
    #[default]
    /// Ascii Table
    Table,
    /// Tab separated
    Tsv,
    Json,
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        std::{path::PathBuf, time::Duration},
    };

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
        assert!(!cfg.signer.sign_only);
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
        assert!(cfg.debug);
        assert!(cfg.mainnet);
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

    #[test]
    fn test_extra_config() -> anyhow::Result<()> {
        let b = "examples/default.toml";
        let cfg = FireblocksConfig::new(b, &[])?;

        // Test extra configuration values from [extra] section
        assert_eq!(cfg.get_extra::<String, _>("rpc_url")?, "https://rpc.com");
        assert!(!cfg.get_extra::<bool, _>("fail_fast")?);
        assert_eq!(cfg.get_extra::<i64, _>("timeout")?, 40);

        // Test with String key (AsRef<str> flexibility)
        let key = String::from("rpc_url");
        assert_eq!(cfg.get_extra::<String, _>(&key)?, "https://rpc.com");

        // Test non-existent key returns NotPresent error
        let result = cfg.get_extra::<String, _>("non_existent");
        assert!(result.is_err());
        if let Err(Error::NotPresent { key }) = result {
            assert_eq!(key, "non_existent");
        } else {
            panic!("Expected NotPresent error");
        }

        // Test has_extra with different key types
        assert!(cfg.has_extra("rpc_url"));
        assert!(cfg.has_extra(String::from("fail_fast")));
        assert!(cfg.has_extra("timeout"));
        assert!(!cfg.has_extra("non_existent"));

        // Test get_extra_duration
        let timeout_duration = cfg.get_extra_duration("timeout")?;
        assert_eq!(timeout_duration, Duration::from_secs(40));

        // Test get_extra_duration with non-existent key
        let result = cfg.get_extra_duration("non_existent");
        assert!(result.is_err());

        Ok(())
    }

    #[test]
    fn test_duration_defaults() -> anyhow::Result<()> {
        let b = "examples/notime.toml";
        let cfg = FireblocksConfig::new(b, &[])?;
        // Verify that string values in TOML are parsed as Duration
        assert_eq!(cfg.signer.poll_timeout, default_poll_timeout());
        assert_eq!(cfg.signer.poll_interval, default_poll_interval());
        Ok(())
    }

    #[test]
    fn test_tilde() -> anyhow::Result<()> {
        let expanded = format!("{}", expand_tilde("~/blah/default.toml").display());
        assert!(expanded.contains("/home"));
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
