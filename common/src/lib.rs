pub mod config;
pub mod constants;
pub mod data;
pub mod fair_channel;
pub mod lease;
pub mod logging;
pub mod protocol;
pub mod proxy_protocol;
pub mod routing;
pub mod transport;
pub mod unix_tcp;
pub mod utils;

include!(concat!(env!("OUT_DIR"), "/build-vars.rs"));

use std::sync::LazyLock;

/// Server domain; baked profile value, overridable at runtime via CLOUDPUB_DOMAIN.
pub static DOMAIN: LazyLock<String> = LazyLock::new(|| {
    std::env::var("CLOUDPUB_DOMAIN").unwrap_or_else(|_| DEFAULT_DOMAIN.to_string())
});

/// Public HTTPS port; baked profile value, overridable at runtime via CLOUDPUB_PORT.
pub static PORT: LazyLock<u16> = LazyLock::new(|| {
    std::env::var("CLOUDPUB_PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_PORT)
});

/// Site display name; baked profile value, overridable at runtime via CLOUDPUB_SITE_NAME.
pub static SITE_NAME: LazyLock<String> = LazyLock::new(|| {
    std::env::var("CLOUDPUB_SITE_NAME").unwrap_or_else(|_| DEFAULT_SITE_NAME.to_string())
});

/// Free-tier traffic limit in bytes; 0 disables the check.
/// Baked profile value, overridable at runtime via CLOUDPUB_TRAFFIC_LIMIT.
#[cfg(target_pointer_width = "64")]
pub static TRAFFIC_LIMIT: LazyLock<usize> = LazyLock::new(|| {
    std::env::var("CLOUDPUB_TRAFFIC_LIMIT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_TRAFFIC_LIMIT)
});

#[cfg(feature = "rustls")]
pub use rustls_pemfile;
#[cfg(feature = "rustls")]
pub use tokio_rustls;
pub use {prost, serde_json, tokio_tungstenite};

#[cfg(test)]
mod build_var_tests {
    // Single test for all statics: LazyLock initializes once per process,
    // so the env must be set before the first access
    #[test]
    fn runtime_env_overrides() {
        std::env::set_var("CLOUDPUB_DOMAIN", "override.example");
        std::env::set_var("CLOUDPUB_PORT", "8443");
        std::env::set_var("CLOUDPUB_SITE_NAME", "Override");
        std::env::set_var("CLOUDPUB_TRAFFIC_LIMIT", "12345");
        assert_eq!(super::DOMAIN.as_str(), "override.example");
        assert_eq!(*super::PORT, 8443);
        assert_eq!(super::SITE_NAME.as_str(), "Override");
        assert_eq!(*super::TRAFFIC_LIMIT, 12345);
    }
}
