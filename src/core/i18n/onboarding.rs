//! First-run source selection, setup and login messages.

#[derive(Debug, Clone, Copy, serde::Serialize)]
pub struct OnboardingStrings {
  pub ok: &'static str,
  pub failed_template: &'static str,
  pub qobuz_logged_in: &'static str,
  pub password: &'static str,
  pub required: &'static str,
}

pub const EN: OnboardingStrings = OnboardingStrings {
  ok: "OK",
  failed_template: "failed: {error}",
  qobuz_logged_in: "Logged in to Qobuz.",
  password: "Password",
  required: "  (required)",
};

// Keep ..EN so new English fields compile before their translations are added.
#[allow(clippy::needless_update)]
pub const PT_BR: OnboardingStrings = OnboardingStrings {
  ok: "OK",
  failed_template: "falhou: {error}",
  qobuz_logged_in: "Conectado ao Qobuz.",
  password: "Senha",
  required: "  (obrigatório)",
  ..EN
};

// Keep ..EN so new English fields compile before their translations are added.
#[allow(clippy::needless_update)]
pub const FR: OnboardingStrings = OnboardingStrings {
  ok: "OK",
  failed_template: "échec : {error}",
  qobuz_logged_in: "Connecté à Qobuz.",
  password: "Mot de passe",
  required: "  (obligatoire)",
  ..EN
};

// The serialized table is shared by all frontends; individual setup flows
// are compiled only when their source or the terminal frontend is enabled.
// Each helper's callers sit behind an optional source or frontend, so only the
// build with all of them (CI's all-sources leg) checks that none is dead.
impl OnboardingStrings {}
