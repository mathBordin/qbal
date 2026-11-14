pub mod audio;
#[cfg(feature = "discord-rpc")]
pub mod discord_rpc;
// The collector half (spawned by the frontend launch) has no caller in a
// headless build; the stats half still serves the CLI. Every tui-enabled CI
// leg lints this module in full.
#[cfg_attr(not(feature = "tui"), allow(dead_code))]
pub mod history;
pub mod media_metadata;
#[cfg(all(feature = "mpris", target_os = "linux"))]
pub mod mpris;
pub mod network;
pub mod qobuz;
pub mod queue;
pub mod redirect_uri;
#[cfg(feature = "scripting")]
pub mod scripting;

/// Read the active language without keeping the app lock alive across a caller's await.
pub(crate) async fn app_strings(
  app: &std::sync::Arc<tokio::sync::Mutex<crate::core::app::App>>,
) -> &'static crate::core::i18n::Strings {
  app.lock().await.strings()
}
