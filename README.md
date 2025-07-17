# fireblocks-config

[![Crates.io](https://img.shields.io/crates/v/fireblocks-config.svg)](https://crates.io/crates/fireblocks-config)
[![Docs.rs](https://docs.rs/fireblocks-config/badge.svg)](https://docs.rs/fireblocks-config)
[![CI](https://github.com/CarteraMesh/fireblocks-config/workflows/CI/badge.svg)](https://github.com/CarteraMesh/fireblocks-config/actions)
[![Cov](https://codecov.io/github/CarteraMesh/fireblocks-config/graph/badge.svg?token=dILa1k9tlW)](https://codecov.io/github/CarteraMesh/fireblocks-config)

## Installation

### Cargo

* Install the rust toolchain in order to have cargo installed by following
  [this](https://www.rust-lang.org/tools/install) guide.
* run `cargo install fireblocks-config`

## Usage

### Creating a Configuration File

Create a TOML configuration file with your Fireblocks settings:

```toml
# config.toml
api_key = "your-api-key-here"
secret_path = "path/to/your/private-key.pem"
url = "https://sandbox-api.fireblocks.io/v1"

[display]
output = "Table"  # Options: Table, Json, Yaml

[signer]
poll_timeout = 120    # Timeout in seconds
poll_interval = 5     # Polling interval in seconds
vault = "0"          # Vault ID
```

### Configuration Overrides

You can layer multiple configuration files for different environments:

**Base configuration** (`config.toml`):
```toml
api_key = "sandbox-key"
secret_path = "keys/sandbox.pem"
url = "https://sandbox-api.fireblocks.io/v1"

[signer]
vault = "0"
```

**Production override** (`prod.toml`):
```toml
api_key = "production-key"
secret_path = "keys/production.pem"
url = "https://api.fireblocks.io/v1"
```

Load with overrides in your code:
```rust,no_run
use fireblocks_config::FireblocksConfig;

// Load base config with production overrides
let config = FireblocksConfig::with_overrides("config.toml", vec!["prod.toml"])?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

### Environment Variables

All configuration values can be overridden using environment variables with the `FIREBLOCKS_` prefix:

```bash
export FIREBLOCKS_API_KEY="your-api-key"
export FIREBLOCKS_SECRET_PATH="/path/to/key.pem"
export FIREBLOCKS_URL="https://api.fireblocks.io/v1"
export FIREBLOCKS_SIGNER__VAULT="1"
export FIREBLOCKS_SIGNER__POLL_TIMEOUT="60"
```

**Note**: Use double underscores (`__`) to access nested configuration sections.

### Alternative: Embedded Secret

Instead of using a file path, you can embed the private key directly in the configuration:

```toml
api_key = "your-api-key"
secret = "-----BEGIN PRIVATE KEY-----\n...\n-----END PRIVATE KEY-----"
url = "https://api.fireblocks.io/v1"
```

### GPG Encrypted Keys

If compiled with the `gpg` feature, you can use GPG-encrypted private key files:

```toml
api_key = "your-api-key"
secret_path = "path/to/encrypted-key.pem.gpg"
url = "https://api.fireblocks.io/v1"
```

### Configuration Priority

Configuration values are loaded in the following order (later values override earlier ones):

1. Base configuration file
2. Override configuration files (in order specified)
3. Environment variables (`FIREBLOCKS_*`)

## Configuration Tips

### Tilde Expansion

The configuration supports `~` for home directory expansion in file paths:

```toml
api_key = "your-api-key"
secret_path = "~/fireblocks/keys/production.pem"
url = "https://api.fireblocks.io/v1"
```

### Vault ID Format

Note that the `vault` field expects a string value, not a number:

```toml
[signer]
vault = "0"    # Correct: string value
# vault = 0    # Incorrect: numeric value
```

## Feature Flags

### XDG Base Directory Support

To enable XDG Base Directory specification support for automatic config loading:

```bash
# Install with XDG support
cargo install fireblocks-config --features xdg

# Or add to Cargo.toml
[dependencies]
fireblocks-config = { version = "0.1", features = ["xdg"] }
```

With XDG support enabled, you can load configs from standard locations:

```rust,no_run
# #[cfg(feature = "xdg")]
# {
use fireblocks_config::FireblocksConfig;

// Load from ~/.config/fireblocks/default.toml
let config = FireblocksConfig::init()?;

// Load default + production profile (using &str)
let config = FireblocksConfig::init_with_profiles(&["production"])?;

// Load with Vec<String> for dynamic profiles
let profiles: Vec<String> = vec!["staging".to_string(), "production".to_string()];
let config = FireblocksConfig::init_with_profiles(&profiles)?;

// Layer multiple profiles: default -> staging -> production
let config = FireblocksConfig::init_with_profiles(&["staging", "production"])?;
# }
# Ok::<(), Box<dyn std::error::Error>>(())
```

**Config file locations:**
- Default: `~/.config/fireblocks/default.toml`
- Profiles: `~/.config/fireblocks/{profile}.toml`

### GPG Support

To enable GPG-encrypted private key support, install with the `gpg` feature:

```bash
# Install with GPG support
cargo install fireblocks-config --features gpg

# Or add to Cargo.toml
[dependencies]
fireblocks-config = { version = "0.1", features = ["gpg"] }
```

With GPG support enabled, you can use encrypted key files:

```toml
api_key = "your-api-key"
secret_path = "path/to/encrypted-key.pem.gpg"
url = "https://api.fireblocks.io/v1"
```

## Development

### Prerequisites

- **Rust Nightly**: Required for code formatting with advanced features
  ```bash
  rustup install nightly
  ```

### Getting Started

1. **Clone the repository**
   ```bash
   git clone https://github.com/CarteraMesh/fireblocks-config.git
   cd fireblocks-config
   ```

2. **Build and test**
   ```bash
   # Build the project
   cargo build

   # Run tests (requires valid Fireblocks credentials in .env)
   cargo test

   # Format code (requires nightly)
   cargo +nightly fmt --all
   ```

### Code Formatting

This project uses advanced Rust formatting features that require nightly:

```bash
# Format all code
cargo +nightly fmt --all

# Check formatting
cargo +nightly fmt --all -- --check
```

## License

 * MIT license
   ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.

See [CONTRIBUTING.md](CONTRIBUTING.md).
