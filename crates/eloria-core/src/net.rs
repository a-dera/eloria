//! Shared HTTPS client (AI, Crossref, Semantic Scholar, OpenAlex).
//!
//! Native system TLS (SChannel on Windows) with the system certificate
//! store: no rustls, hence no `ring` (which needs clang on Windows ARM64).
//! Both settings are required: by default, ureq picks rustls and its own
//! certificate list, which fails at runtime with only `native-tls`.

use std::time::Duration;

use ureq::tls::{RootCerts, TlsConfig, TlsProvider};

/// HTTPS agent. `total` bounds the total duration of a request (reading the stream included).
/// HTTP error codes are not turned into errors: the caller reads the body.
pub fn agent(total: Duration) -> ureq::Agent {
    ureq::Agent::config_builder()
        .tls_config(TlsConfig::builder().provider(TlsProvider::NativeTls).root_certs(RootCerts::PlatformVerifier).build())
        .timeout_connect(Some(Duration::from_secs(20)))
        .timeout_global(Some(total))
        .http_status_as_error(false)
        .user_agent(concat!("Eloria/", env!("CARGO_PKG_VERSION")))
        .build()
        .into()
}
