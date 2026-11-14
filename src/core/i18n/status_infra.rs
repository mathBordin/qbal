//! User-visible infrastructure status messages. Raw diagnostics remain untranslated.

#[derive(Debug, Clone, Copy, serde::Serialize)]
pub struct InfraStatusStrings {
  pub history_track_hit: &'static str,
  pub history_play: &'static str,
  pub audio_init_failed: &'static str,
  pub qobuz_audio_unavailable: &'static str,
  pub qobuz_login_expired: &'static str,
  pub qobuz_login_busy: &'static str,
  pub qobuz_browser_login: &'static str,
  pub qobuz_logged_in: &'static str,
  pub qobuz_no_tracks: &'static str,
  pub qobuz_uri_invalid: &'static str,
  pub qobuz_replay_failed: &'static str,
  pub qobuz_login_url: &'static str,
  pub qobuz_login_failed: &'static str,
  pub qobuz_step_constants: &'static str,
  pub qobuz_step_login: &'static str,
  pub qobuz_step_library: &'static str,
  pub qobuz_step_tracks: &'static str,
  pub qobuz_step_search: &'static str,
  pub qobuz_step_stream: &'static str,
  pub qobuz_operation_failed: &'static str,
  pub qobuz_no_full_stream: &'static str,
  pub music_queue_unsupported: &'static str,
  pub queue_repeat_unsupported: &'static str,
  pub queue_shuffle_unsupported: &'static str,
  pub queue_no_uri: &'static str,
  pub queue_finished: &'static str,
  pub audio_disconnected: &'static str,
  pub source_playback_unavailable: &'static str,
  pub queue_play_failed: &'static str,
  pub queue_playing: &'static str,
  pub queue_audio_unavailable: &'static str,
  pub spotify_not_connected: &'static str,
  pub recap_created: &'static str,
  pub history_failed: &'static str,
  pub recap_failed: &'static str,
  pub history_plays: &'static str,
  pub history_track_hits: &'static str,
  pub history_minutes: &'static str,
  pub history_hours_minutes: &'static str,
  pub history_week: &'static str,
  pub history_month: &'static str,
  pub history_all: &'static str,
  pub history_this_month: &'static str,
  pub history_this_year: &'static str,
  pub monthly_recap_ready: &'static str,
  pub queue_local_source: &'static str,
  pub local_root_folder: &'static str,
  pub local_unknown_track: &'static str,
}

// Each helper's callers sit behind an optional source or frontend, so only the
// build with all of them (CI's all-sources leg) checks that none is dead.
impl InfraStatusStrings {
  pub fn audio_init_failed(&self, error: &str) -> String {
    super::format_template(self.audio_init_failed, &[("error", error)])
  }
  pub fn qobuz_audio_unavailable(&self, error: &str) -> String {
    super::format_template(self.qobuz_audio_unavailable, &[("error", error)])
  }
  pub fn qobuz_login_url(&self, url: &str) -> String {
    super::format_template(self.qobuz_login_url, &[("url", url)])
  }
  pub fn qobuz_login_failed(&self, error: &str) -> String {
    super::format_template(self.qobuz_login_failed, &[("error", error)])
  }
  pub fn qobuz_operation_failed(&self, step: &str, error: &str) -> String {
    super::format_template(
      self.qobuz_operation_failed,
      &[("step", step), ("error", error)],
    )
  }
  pub fn source_playback_unavailable(&self, source: &str) -> String {
    super::format_template(self.source_playback_unavailable, &[("source", source)])
  }
  pub fn queue_play_failed(&self, name: &str, error: &str) -> String {
    super::format_template(self.queue_play_failed, &[("name", name), ("error", error)])
  }
  pub fn queue_playing(&self, name: &str) -> String {
    super::format_template(self.queue_playing, &[("name", name)])
  }
  pub fn queue_audio_unavailable(&self, error: &str) -> String {
    super::format_template(self.queue_audio_unavailable, &[("error", error)])
  }
  pub fn recap_created(&self, path: &str, count: usize) -> String {
    super::format_template(
      self.recap_created,
      &[("path", path), ("count", &count.to_string())],
    )
  }
  pub fn history_failed(&self, error: &str) -> String {
    super::format_template(self.history_failed, &[("error", error)])
  }
  pub fn recap_failed(&self, error: &str) -> String {
    super::format_template(self.recap_failed, &[("error", error)])
  }
  pub fn history_plays(&self, count: u64, duration: &str) -> String {
    super::format_template(
      if count == 1 {
        self.history_play
      } else {
        self.history_plays
      },
      &[("count", &count.to_string()), ("duration", duration)],
    )
  }
  pub fn history_track_hits(&self, count: u64, duration: &str) -> String {
    super::format_template(
      if count == 1 {
        self.history_track_hit
      } else {
        self.history_track_hits
      },
      &[("count", &count.to_string()), ("duration", duration)],
    )
  }
  pub fn history_minutes(&self, minutes: u64) -> String {
    super::format_template(self.history_minutes, &[("minutes", &minutes.to_string())])
  }
  pub fn history_hours_minutes(&self, hours: u64, minutes: u64) -> String {
    super::format_template(
      self.history_hours_minutes,
      &[
        ("hours", &hours.to_string()),
        ("minutes", &minutes.to_string()),
      ],
    )
  }
  pub fn monthly_recap_ready(&self, path: &str, count: usize) -> String {
    super::format_template(
      self.monthly_recap_ready,
      &[("path", path), ("count", &count.to_string())],
    )
  }
}

pub const EN: InfraStatusStrings = InfraStatusStrings {
  history_track_hit: "{count} track hit · {duration}",
  history_play: "{count} play · {duration}",
  audio_init_failed: "Audio output init failed: {error}",
  qobuz_audio_unavailable: "No audio output for Qobuz playback: {error}",
  qobuz_login_expired: "Qobuz: login expired, press `d` and pick Qobuz to log in again",
  qobuz_login_busy: "Qobuz login already in progress...",
  qobuz_browser_login: "Qobuz: open the browser window to log in",
  qobuz_logged_in: "Qobuz: logged in",
  qobuz_no_tracks: "No Qobuz tracks to play",
  qobuz_uri_invalid: "Invalid Qobuz track URI",
  qobuz_replay_failed: "Cannot replay Qobuz track",
  qobuz_login_url: "Open this URL in your browser to log in to Qobuz: {url}",
  qobuz_login_failed: "Qobuz login failed: {error}",
  qobuz_step_constants: "web player constants",
  qobuz_step_login: "login",
  qobuz_step_library: "library",
  qobuz_step_tracks: "tracks",
  qobuz_step_search: "search",
  qobuz_step_stream: "stream",
  qobuz_operation_failed: "Qobuz: {step}: {error}",
  qobuz_no_full_stream:
    "Qobuz did not offer this track in full: streaming needs a paid Qobuz subscription",
  music_queue_unsupported: "Apple Music tracks can't be queued",
  queue_repeat_unsupported: "Repeat does not apply to this source",
  queue_shuffle_unsupported: "Shuffle does not apply to this source",
  queue_no_uri: "Skipped a queued track with no URI",
  queue_finished: "Queue finished",
  audio_disconnected: "Audio output device disconnected.",
  source_playback_unavailable: "{source} playback isn't available in this build",
  queue_play_failed: "Cannot play {name}: {error}",
  queue_playing: "♪ {name} (queue)",
  queue_audio_unavailable: "No audio output for queue playback: {error}",
  spotify_not_connected: "Spotify not connected. Press `d` and pick Spotify to log in.",
  recap_created: "Listening recap generated at {path} ({count} listens)",
  history_failed: "failed to load listening history: {error}",
  recap_failed: "Failed to generate recap: {error}",
  history_plays: "{count} plays · {duration}",
  history_track_hits: "{count} track hits · {duration}",
  history_minutes: "{minutes}m",
  history_hours_minutes: "{hours}h {minutes}m",
  history_week: "Last 7 Days",
  history_month: "Last 30 Days",
  history_all: "All Time",
  history_this_month: "This Month",
  history_this_year: "This Year",
  monthly_recap_ready: "Monthly listening recap ready at {path} ({count} listens)",
  queue_local_source: "Local",
  local_root_folder: "(root)",
  local_unknown_track: "Unknown Track",
};

// Keep ..EN so new English fields compile before their translations are added.
#[allow(clippy::needless_update)]
pub const PT_BR: InfraStatusStrings = InfraStatusStrings {
  history_track_hit: "{count} faixa · {duration}",
  history_play: "{count} reprodução · {duration}",
  audio_init_failed: "Falha ao iniciar a saída de áudio: {error}",
  qobuz_audio_unavailable: "Sem saída de áudio para tocar o Qobuz: {error}",
  qobuz_login_expired: "Qobuz: o login expirou, entre novamente",
  qobuz_login_busy: "Login no Qobuz já em andamento...",
  qobuz_browser_login: "Qobuz: abra a janela do navegador para entrar",
  qobuz_logged_in: "Qobuz: conectado",
  qobuz_no_tracks: "Nenhuma faixa do Qobuz para tocar",
  qobuz_uri_invalid: "URI de faixa do Qobuz inválida",
  qobuz_replay_failed: "Não é possível repetir a faixa do Qobuz",
  qobuz_login_url: "Abra este URL no navegador para entrar no Qobuz: {url}",
  qobuz_login_failed: "Falha no login do Qobuz: {error}",
  qobuz_step_constants: "constantes do web player",
  qobuz_step_login: "login",
  qobuz_step_library: "biblioteca",
  qobuz_step_tracks: "faixas",
  qobuz_step_search: "busca",
  qobuz_step_stream: "stream",
  qobuz_operation_failed: "Qobuz: {step}: {error}",
  qobuz_no_full_stream:
    "O Qobuz não ofereceu esta faixa completa: o streaming exige uma assinatura paga do Qobuz",
  music_queue_unsupported: "Faixas do Apple Music não podem entrar na fila",
  queue_repeat_unsupported: "Repetir não se aplica a esta fonte",
  queue_shuffle_unsupported: "O modo aleatório não se aplica a esta fonte",
  queue_no_uri: "Uma faixa da fila sem URI foi pulada",
  queue_finished: "Fila concluída",
  audio_disconnected: "Dispositivo de saída de áudio desconectado.",
  source_playback_unavailable: "A reprodução de {source} não está disponível nesta versão",
  queue_play_failed: "Não é possível tocar {name}: {error}",
  queue_playing: "♪ {name} (fila)",
  queue_audio_unavailable: "Sem saída de áudio para tocar a fila: {error}",
  spotify_not_connected: "Spotify não conectado.",
  recap_created: "Retrospectiva gerada em {path} ({count} reproduções)",
  history_failed: "falha ao carregar o histórico de reprodução: {error}",
  recap_failed: "Falha ao gerar a retrospectiva: {error}",
  history_plays: "{count} reproduções · {duration}",
  history_track_hits: "{count} faixas · {duration}",
  history_minutes: "{minutes}min",
  history_hours_minutes: "{hours}h {minutes}min",
  history_week: "Últimos 7 dias",
  history_month: "Últimos 30 dias",
  history_all: "Desde sempre",
  history_this_month: "Este mês",
  history_this_year: "Este ano",
  monthly_recap_ready: "Retrospectiva mensal pronta em {path} ({count} reproduções)",
  queue_local_source: "Local",
  local_root_folder: "(raiz)",
  local_unknown_track: "Faixa desconhecida",
  ..EN
};

// Keep ..EN so new English fields compile before their translations are added.
#[allow(clippy::needless_update)]
pub const FR: InfraStatusStrings = InfraStatusStrings {
  history_track_hit: "{count} titre · {duration}",
  history_play: "{count} écoute · {duration}",
  audio_init_failed: "Échec de l'initialisation de la sortie audio : {error}",
  qobuz_audio_unavailable: "Aucune sortie audio pour la lecture Qobuz : {error}",
  qobuz_login_expired: "Qobuz : la session a expiré, reconnectez-vous",
  qobuz_login_busy: "Connexion à Qobuz déjà en cours...",
  qobuz_browser_login: "Qobuz : ouvrez la fenêtre du navigateur pour vous connecter",
  qobuz_logged_in: "Qobuz : connecté",
  qobuz_no_tracks: "Aucun titre Qobuz à lire",
  qobuz_uri_invalid: "URI de titre Qobuz invalide",
  qobuz_replay_failed: "Impossible de relire le titre Qobuz",
  qobuz_login_url: "Ouvrez cette URL dans votre navigateur pour vous connecter à Qobuz : {url}",
  qobuz_login_failed: "Échec de la connexion à Qobuz : {error}",
  qobuz_step_constants: "constantes du lecteur web",
  qobuz_step_login: "connexion",
  qobuz_step_library: "bibliothèque",
  qobuz_step_tracks: "titres",
  qobuz_step_search: "recherche",
  qobuz_step_stream: "flux",
  qobuz_operation_failed: "Qobuz : {step} : {error}",
  qobuz_no_full_stream:
    "Qobuz n'a pas fourni ce titre en entier : le streaming nécessite un abonnement Qobuz payant",
  music_queue_unsupported: "Les titres Apple Music ne peuvent pas être ajoutés à la file",
  queue_repeat_unsupported: "La répétition ne s'applique pas à cette source",
  queue_shuffle_unsupported: "La lecture aléatoire ne s'applique pas à cette source",
  queue_no_uri: "Un titre de la file sans URI a été ignoré",
  queue_finished: "File terminée",
  audio_disconnected: "Périphérique de sortie audio déconnecté.",
  source_playback_unavailable: "La lecture de {source} n'est pas disponible dans cette version",
  queue_play_failed: "Impossible de lire {name} : {error}",
  queue_playing: "♪ {name} (file)",
  queue_audio_unavailable: "Aucune sortie audio pour la lecture de la file : {error}",
  spotify_not_connected: "Spotify non connecté.",
  recap_created: "Bilan d'écoute généré dans {path} ({count} écoutes)",
  history_failed: "échec du chargement de l'historique d'écoute : {error}",
  recap_failed: "Échec de la génération du bilan : {error}",
  history_plays: "{count} écoutes · {duration}",
  history_track_hits: "{count} titres · {duration}",
  history_minutes: "{minutes} min",
  history_hours_minutes: "{hours} h {minutes} min",
  history_week: "7 derniers jours",
  history_month: "30 derniers jours",
  history_all: "Depuis le début",
  history_this_month: "Ce mois-ci",
  history_this_year: "Cette année",
  monthly_recap_ready: "Bilan mensuel prêt dans {path} ({count} écoutes)",
  queue_local_source: "Local",
  local_root_folder: "(racine)",
  local_unknown_track: "Titre inconnu",
  ..EN
};
