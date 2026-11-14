//! Localized application messages.
#[derive(Debug, Clone, Copy, serde::Serialize)]
pub struct AppStatusStrings {
  pub audio_device_paused: &'static str,
  pub audio_device_moved: &'static str,
  pub audio_device_waiting: &'static str,

  pub loading_playlist: &'static str,
  pub settings_shortcut_fallback: &'static str,
  pub radio_no_url: &'static str,
  pub queue_no_uri: &'static str,
  pub shuffle_on: &'static str,
  pub shuffle_off: &'static str,
  pub repeat_off: &'static str,
  pub repeat_all: &'static str,
  pub repeat_one: &'static str,
  pub shuffle_unsupported: &'static str,
  pub repeat_unsupported: &'static str,
  pub recap_disabled: &'static str,
  pub nothing_playing: &'static str,
  pub spotify_not_connected: &'static str,
  pub party_needs_spotify: &'static str,
  pub setting_locked_from_page: &'static str,
  pub state_save_failed: &'static str,
  pub shortcut_save_failed: &'static str,
  pub shortcut_saved: &'static str,
  pub queued: &'static str,
  pub recap_browser_failed: &'static str,
  pub entry_not_playable: &'static str,
  pub liked_loading: &'static str,
  pub no_radio_selected: &'static str,
  pub announcement_save_failed: &'static str,
  pub link_open_failed: &'static str,
  pub capture_unavailable: &'static str,
  pub search_needs_spotify: &'static str,
  pub shortcut_session_only: &'static str,
  pub search_source_unavailable: &'static str,
  pub paused: &'static str,
  pub paused_details: &'static str,
  pub visualizer_config: &'static str,
  pub pane_nothing_to_detach: &'static str,
  pub pane_hidden: &'static str,
  pub pane_no_room: &'static str,
}

pub const EN: AppStatusStrings = AppStatusStrings {
  audio_device_paused: "Audio device changed - playback paused here.",
  audio_device_moved: "Audio device changed - playback moved to it.",
  audio_device_waiting: "Audio output device lost - waiting for a new one.",

  loading_playlist: "Loading playlist…",
  settings_shortcut_fallback: "Ctrl+, not detected in this terminal; using Alt+, for this session",
  radio_no_url: "Radio station has no stream URL",
  queue_no_uri: "Cannot queue: track has no URI",
  shuffle_on: "Shuffle: On",
  shuffle_off: "Shuffle: Off",
  repeat_off: "Repeat: Off",
  repeat_all: "Repeat: All",
  repeat_one: "Repeat: One",
  shuffle_unsupported: "Shuffle does not apply to this source",
  repeat_unsupported: "Repeat does not apply to this source",
  recap_disabled: "Monthly recap prompt disabled (re-enable in Settings)",
  nothing_playing: "Nothing is playing",
  spotify_not_connected: "Spotify not connected. Press `d` and pick Spotify to log in.",
  party_needs_spotify: "Listening Party needs Spotify. Press `d` and pick Spotify to log in.",
  setting_locked_from_page: "The browser page cannot set a program to run. Change this path in config.yml or in the terminal Settings.",
  state_save_failed: "Failed to save state: {error}",
  shortcut_save_failed: "Failed to save keybinding fallback: {error}",
  shortcut_saved: "Saved open settings shortcut as {key}",
  queued: "Queued: {name}",
  recap_browser_failed: "Recap saved at {path} (couldn't open browser)",
  entry_not_playable: "This entry can't be played directly",
  liked_loading: "Liked songs are still loading, please wait...",
  no_radio_selected: "No radio station selected",
  announcement_save_failed: "Failed to persist dismissed announcement: {error}",
  link_open_failed: "Couldn't open {url} in the browser",
  capture_unavailable: "Visualizer can't capture system audio - see the log for details.",
  search_needs_spotify: "Search needs a Spotify session",
  shortcut_session_only: "Using Alt+, for this session only",
  search_source_unavailable: "Search isn't available for {source}",
  paused: "Paused",
  paused_details: "Paused: {details}",
  visualizer_config: "visualizer.yml: {problem}",
  pane_nothing_to_detach:
    "Open lyrics, the visualizer, cover art, the queue or the mini player, then press {key} to detach it",
  pane_hidden: "The terminal is too small to show the panes. Press {key} to close one",
  pane_no_room: "There is no room for another pane in this terminal",
};

// Keep ..EN so new English fields compile before their translations are added.
#[allow(clippy::needless_update)]
pub const PT_BR: AppStatusStrings = AppStatusStrings {
  audio_device_paused: "O dispositivo de áudio mudou - reprodução pausada aqui.",
  audio_device_moved: "O dispositivo de áudio mudou - a reprodução foi para ele.",
  audio_device_waiting: "Saída de áudio perdida - aguardando uma nova.",
  loading_playlist: "Carregando playlist…",
  settings_shortcut_fallback: "Ctrl+, não foi detectado neste terminal; usando Alt+, nesta sessão",
  radio_no_url: "A rádio não tem URL de stream",
  queue_no_uri: "Não é possível adicionar à fila: a faixa não tem URI",
  shuffle_on: "Aleatório: ligado",
  shuffle_off: "Aleatório: desligado",
  repeat_off: "Repetir: desligado",
  repeat_all: "Repetir: tudo",
  repeat_one: "Repetir: uma",
  shuffle_unsupported: "O modo aleatório não se aplica a esta fonte",
  repeat_unsupported: "Repetir não se aplica a esta fonte",
  recap_disabled: "Aviso de retrospectiva mensal desativado (reative em Configurações)",
  nothing_playing: "Nada está tocando",
  spotify_not_connected: "Spotify não conectado.",
  party_needs_spotify: "A sessão de escuta precisa do Spotify.",
  setting_locked_from_page: "A página do navegador não pode definir um programa a executar. Altere este caminho no config.yml ou nas Configurações do terminal.",
  state_save_failed: "Falha ao salvar o estado: {error}",
  shortcut_save_failed: "Falha ao salvar o atalho alternativo: {error}",
  shortcut_saved: "Atalho para abrir as configurações salvo como {key}",
  queued: "Na fila: {name}",
  recap_browser_failed: "Retrospectiva salva em {path} (não foi possível abrir o navegador)",
  entry_not_playable: "Este item não pode ser tocado diretamente",
  liked_loading: "As músicas curtidas ainda estão carregando, aguarde...",
  no_radio_selected: "Nenhuma rádio selecionada",
  announcement_save_failed: "Falha ao salvar o aviso dispensado: {error}",
  link_open_failed: "Não foi possível abrir {url} no navegador",
  capture_unavailable: "O visualizador não consegue capturar o áudio do sistema - veja o log para detalhes.",
  search_needs_spotify: "A busca precisa de uma sessão do Spotify",
  shortcut_session_only: "Usando Alt+, apenas nesta sessão",
  search_source_unavailable: "A busca não está disponível para {source}",
  paused: "Pausado",
  paused_details: "Pausado: {details}",
  visualizer_config: "visualizer.yml: {problem}",
  pane_nothing_to_detach: "Abra a letra, o visualizador, a capa, a fila ou o miniplayer e pressione {key} para destacá-lo",
  pane_hidden: "O terminal é pequeno demais para mostrar os painéis. Pressione {key} para fechar um",
  pane_no_room: "Não há espaço para outro painel neste terminal",
  ..EN
};

// Keep ..EN so new English fields compile before their translations are added.
#[allow(clippy::needless_update)]
pub const FR: AppStatusStrings = AppStatusStrings {
  audio_device_paused: "Le périphérique audio a changé - lecture mise en pause ici.",
  audio_device_moved: "Le périphérique audio a changé - la lecture y a été déplacée.",
  audio_device_waiting: "Sortie audio perdue - en attente d'une nouvelle.",
  loading_playlist: "Chargement de la playlist…",
  settings_shortcut_fallback: "Ctrl+, non détecté dans ce terminal ; Alt+, est utilisé pour cette session",
  radio_no_url: "La station de radio n'a pas d'URL de flux",
  queue_no_uri: "Impossible d'ajouter à la file : le titre n'a pas d'URI",
  shuffle_on: "Lecture aléatoire : activée",
  shuffle_off: "Lecture aléatoire : désactivée",
  repeat_off: "Répétition : désactivée",
  repeat_all: "Répétition : tout",
  repeat_one: "Répétition : un titre",
  shuffle_unsupported: "La lecture aléatoire ne s'applique pas à cette source",
  repeat_unsupported: "La répétition ne s'applique pas à cette source",
  recap_disabled: "Rappel du bilan mensuel désactivé (réactivez-le dans les Paramètres)",
  nothing_playing: "Aucune lecture en cours",
  spotify_not_connected: "Spotify non connecté.",
  party_needs_spotify: "La session d'écoute nécessite Spotify.",
  setting_locked_from_page: "La page du navigateur ne peut pas définir un programme à exécuter. Modifiez ce chemin dans config.yml ou dans les Paramètres du terminal.",
  state_save_failed: "Échec de l'enregistrement de l'état : {error}",
  shortcut_save_failed: "Échec de l'enregistrement du raccourci de secours : {error}",
  shortcut_saved: "Raccourci d'ouverture des paramètres enregistré : {key}",
  queued: "Ajouté à la file : {name}",
  recap_browser_failed: "Bilan enregistré dans {path} (impossible d'ouvrir le navigateur)",
  entry_not_playable: "Cet élément ne peut pas être lu directement",
  liked_loading: "Les titres likés sont en cours de chargement, patientez...",
  no_radio_selected: "Aucune station de radio sélectionnée",
  announcement_save_failed: "Échec de l'enregistrement de l'annonce ignorée : {error}",
  link_open_failed: "Impossible d'ouvrir {url} dans le navigateur",
  capture_unavailable: "Le visualiseur ne peut pas capturer l'audio du système - consultez le journal pour plus de détails.",
  search_needs_spotify: "La recherche nécessite une session Spotify",
  shortcut_session_only: "Alt+, est utilisé pour cette session uniquement",
  search_source_unavailable: "La recherche n'est pas disponible pour {source}",
  paused: "En pause",
  paused_details: "En pause : {details}",
  visualizer_config: "visualizer.yml : {problem}",
  pane_nothing_to_detach: "Ouvrez les paroles, le visualiseur, la pochette, la file ou le mini-lecteur, puis appuyez sur {key} pour le détacher",
  pane_hidden: "Le terminal est trop petit pour afficher les panneaux. Appuyez sur {key} pour en fermer un",
  pane_no_room: "Il n'y a pas de place pour un autre panneau dans ce terminal",
  ..EN
};

// Each helper's callers sit behind an optional source or frontend, so only the
// build with all of them (CI's all-sources leg) checks that none is dead.
impl AppStatusStrings {
  pub fn state_save_failed(&self, error: &str) -> String {
    crate::core::i18n::format_template(self.state_save_failed, &[("error", error)])
  }
  pub fn shortcut_save_failed(&self, error: &str) -> String {
    crate::core::i18n::format_template(self.shortcut_save_failed, &[("error", error)])
  }
  pub fn shortcut_saved(&self, key: &str) -> String {
    crate::core::i18n::format_template(self.shortcut_saved, &[("key", key)])
  }
  pub fn queued(&self, name: &str) -> String {
    crate::core::i18n::format_template(self.queued, &[("name", name)])
  }
  pub fn recap_browser_failed(&self, path: &str) -> String {
    crate::core::i18n::format_template(self.recap_browser_failed, &[("path", path)])
  }
  pub fn announcement_save_failed(&self, error: &str) -> String {
    crate::core::i18n::format_template(self.announcement_save_failed, &[("error", error)])
  }
  pub fn link_open_failed(&self, url: &str) -> String {
    crate::core::i18n::format_template(self.link_open_failed, &[("url", url)])
  }
  pub fn search_source_unavailable(&self, source: &str) -> String {
    crate::core::i18n::format_template(self.search_source_unavailable, &[("source", source)])
  }
  pub fn paused_details(&self, details: &str) -> String {
    crate::core::i18n::format_template(self.paused_details, &[("details", details)])
  }
  pub fn pane_nothing_to_detach(&self, key: &str) -> String {
    crate::core::i18n::format_template(self.pane_nothing_to_detach, &[("key", key)])
  }
  pub fn pane_hidden(&self, key: &str) -> String {
    crate::core::i18n::format_template(self.pane_hidden, &[("key", key)])
  }
}
