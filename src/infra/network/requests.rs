//! The process-wide HTTP client shared by the lyrics, telemetry and
//! announcement requests.

use std::sync::OnceLock;
use std::time::Duration;

static SHARED_HTTP_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

/// Returns the process-wide shared [`reqwest::Client`].
///
/// A `reqwest::Client` owns a connection pool and is meant to be built once and
/// reused. Every caller gets bounded, non-streaming responses, so a blanket
/// request timeout is safe: without one, a post-connect read stall (captive
/// portal, half-open TCP) would make `send()` await forever. An explicit
/// connect timeout additionally bounds the TCP/TLS handshake.
pub fn shared_http_client() -> &'static reqwest::Client {
  SHARED_HTTP_CLIENT.get_or_init(|| {
    reqwest::Client::builder()
      .connect_timeout(Duration::from_secs(10))
      .timeout(Duration::from_secs(30))
      .build()
      .unwrap_or_else(|_| reqwest::Client::new())
  })
}
