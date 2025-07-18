use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("config not found {path}")]
    ConfigNotFound { path: String },

    #[error(transparent)]
    ConfigParseError(#[from] config::ConfigError),

    #[error(transparent)]
    IO(#[from] std::io::Error),

    #[error("missing secret key. Check your configuration file or set env FIREBLOCKS_SECRET")]
    MissingSecret,

    #[error("{asset} not found")]
    AssetNotFound { asset: String },

    #[error("IO error for file {path:?}: {source}")]
    IOError {
        source: std::io::Error,
        path: String,
    },

    #[error("Invalid Duration {0}")]
    InvalidDuration(String),

    #[error("Key '{key}' not present in configuration")]
    NotPresent { key: String },

    #[cfg(feature = "gpg")]
    #[error(transparent)]
    GpgError(#[from] gpgme::Error),

    #[cfg(feature = "xdg")]
    #[error(transparent)]
    XdgError(#[from] microxdg::XdgError),
}
