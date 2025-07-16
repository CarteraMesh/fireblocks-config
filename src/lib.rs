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
