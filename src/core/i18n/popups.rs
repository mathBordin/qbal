//! Popup and dialog copy. User names, titles, paths, codes and URLs are inserted literally.

#[derive(Debug, Clone, Copy, serde::Serialize)]
pub struct PopupStrings {
  pub now_playing: &'static str,
  pub queue_empty_template: &'static str,
  pub queue_up_next: &'static str,
  pub queue_title_template: &'static str,
  pub api_response: &'static str,
  pub playback_check: &'static str,
  pub check_subscription: &'static str,
  pub check_audio: &'static str,
  pub playback_hint: &'static str,
  pub error_back: &'static str,
  pub error_title: &'static str,
  pub log_hint: &'static str,
  pub log_file: &'static str,
  pub ok: &'static str,
  pub announcement_info: &'static str,
  pub announcement_warning: &'static str,
  pub announcement_critical: &'static str,
  pub announcement_more_template: &'static str,
  pub announcement_dismiss: &'static str,
  pub announcement_title: &'static str,
  pub recap_ready: &'static str,
  pub recap_listens_template: &'static str,
  pub recap_browser: &'static str,
  pub recap_controls: &'static str,
  pub recap_title: &'static str,
  pub exit_question: &'static str,
  pub exit_yes_no: &'static str,
  pub exit_controls: &'static str,
  pub exit_title: &'static str,
  pub sort_tracks: &'static str,
  pub sort_default: &'static str,
  pub sort_name: &'static str,
  pub sort_artist: &'static str,
  pub sort_duration: &'static str,
  pub sort_album: &'static str,
  pub sort_release_date: &'static str,
  pub plugin_close_hint: &'static str,
}

pub const EN: PopupStrings = PopupStrings {
  now_playing: "Now playing: ",
  queue_empty_template: "Queue is empty — press {key} on a track to add it",
  queue_up_next: "Up next from context:",
  queue_title_template: "Queue  ({key} remove · J/K move · Enter play · Esc back)",
  api_response: "Error: ",
  playback_check: "If playback fails, check that",
  check_subscription: " 1. You are signed in to Qobuz and your subscription is active",
  check_audio: " 2. Your computer's audio output works",
  playback_hint: "Hint: the log file below has the details",
  error_back: "\nPress <Esc> to return",
  error_title: "Error",
  log_hint: "Rerun with --debug for more detail. Read it before posting publicly.",
  log_file: "Log file: ",
  ok: "Ok",
  announcement_info: "INFO",
  announcement_warning: "WARNING",
  announcement_critical: "CRITICAL",
  announcement_more_template: "More: {url}",
  announcement_dismiss: "[Press ENTER or ESC to dismiss]",
  announcement_title: " Announcement ",
  recap_ready: "Your monthly listening recap is ready! 🎉",
  recap_listens_template: "{count} listens made it into your 30-day recap.",
  recap_browser: "Open it in the browser to view and download your share card.",
  recap_controls: "[ENTER] Open   [ESC] Later   [d] Don't show this again",
  recap_title: " Monthly Recap ",
  exit_question: "Exit Qbal?",
  exit_yes_no: "Press Y for Yes or N for No",
  exit_controls: "[ENTER = Yes, ESC = No]",
  exit_title: " Confirm Exit ",
  sort_tracks: "Sort Tracks",
  sort_default: "Default",
  sort_name: "Name",
  sort_artist: "Artist",
  sort_duration: "Duration",
  sort_album: "Album",
  sort_release_date: "Release Date",
  plugin_close_hint: "(Esc to close)",
};

// Keep ..EN so new English fields compile before their translations are added.
#[allow(clippy::needless_update)]
pub const PT_BR: PopupStrings = PopupStrings {
  now_playing: "Tocando agora: ",
  queue_empty_template: "A fila está vazia — pressione {key} em uma faixa para adicioná-la",
  queue_up_next: "A seguir no contexto:",
  queue_title_template: "Fila  ({key} remover · J/K mover · Enter tocar · Esc voltar)",
  api_response: "Erro: ",
  playback_check: "Se a reprodução falhar, verifique se",
  check_subscription: " 1. Você está conectado ao Qobuz e sua assinatura está ativa",
  check_audio: " 2. A saída de áudio do computador funciona",
  playback_hint: "Dica: o arquivo de log abaixo tem os detalhes",
  error_back: "\nPressione <Esc> para voltar",
  error_title: "Erro",
  log_hint: "Execute novamente com --debug para mais detalhes. Leia antes de publicar.",
  log_file: "Arquivo de log: ",
  ok: "Ok",
  announcement_info: "INFO",
  announcement_warning: "AVISO",
  announcement_critical: "CRÍTICO",
  announcement_more_template: "Mais: {url}",
  announcement_dismiss: "[Pressione ENTER ou ESC para dispensar]",
  announcement_title: " Aviso ",
  recap_ready: "Sua retrospectiva mensal está pronta! 🎉",
  recap_listens_template: "{count} reproduções entraram na sua retrospectiva de 30 dias.",
  recap_browser: "Abra no navegador para ver e baixar seu cartão para compartilhar.",
  recap_controls: "[ENTER] Abrir   [ESC] Depois   [d] Não mostrar novamente",
  recap_title: " Retrospectiva mensal ",
  exit_question: "Sair do Qbal?",
  exit_yes_no: "Pressione Y para Sim ou N para Não",
  exit_controls: "[ENTER = Sim, ESC = Não]",
  exit_title: " Confirmar saída ",
  sort_tracks: "Ordenar faixas",
  sort_default: "Padrão",
  sort_name: "Nome",
  sort_artist: "Artista",
  sort_duration: "Duração",
  sort_album: "Álbum",
  sort_release_date: "Data de lançamento",
  plugin_close_hint: "(Esc para fechar)",
  ..EN
};

// Keep ..EN so new English fields compile before their translations are added.
#[allow(clippy::needless_update)]
pub const FR: PopupStrings = PopupStrings {
  now_playing: "Lecture en cours : ",
  queue_empty_template: "La file est vide — appuyez sur {key} sur un titre pour l'ajouter",
  queue_up_next: "À suivre dans le contexte :",
  queue_title_template: "File  ({key} retirer · J/K déplacer · Entrée lire · Échap retour)",
  api_response: "Erreur : ",
  playback_check: "Si la lecture échoue, vérifiez que",
  check_subscription: " 1. Vous êtes connecté à Qobuz et votre abonnement est actif",
  check_audio: " 2. La sortie audio de l'ordinateur fonctionne",
  playback_hint: "Astuce : le fichier journal ci-dessous donne les détails",
  error_back: "\nAppuyez sur <Échap> pour revenir",
  error_title: "Erreur",
  log_hint: "Relancez avec --debug pour plus de détails. Relisez-le avant de le publier.",
  log_file: "Fichier journal : ",
  ok: "Ok",
  announcement_info: "INFO",
  announcement_warning: "AVERTISSEMENT",
  announcement_critical: "CRITIQUE",
  announcement_more_template: "Plus : {url}",
  announcement_dismiss: "[Appuyez sur ENTRÉE ou ÉCHAP pour fermer]",
  announcement_title: " Annonce ",
  recap_ready: "Votre bilan d'écoute mensuel est prêt ! 🎉",
  recap_listens_template: "{count} écoutes figurent dans votre bilan sur 30 jours.",
  recap_browser: "Ouvrez-le dans le navigateur pour voir et télécharger votre carte à partager.",
  recap_controls: "[ENTRÉE] Ouvrir   [ÉCHAP] Plus tard   [d] Ne plus afficher",
  recap_title: " Bilan mensuel ",
  exit_question: "Quitter Qbal ?",
  exit_yes_no: "Appuyez sur Y pour Oui ou N pour Non",
  exit_controls: "[ENTRÉE = Oui, ÉCHAP = Non]",
  exit_title: " Confirmer la sortie ",
  sort_tracks: "Trier les titres",
  sort_default: "Par défaut",
  sort_name: "Nom",
  sort_artist: "Artiste",
  sort_duration: "Durée",
  sort_album: "Album",
  sort_release_date: "Date de sortie",
  plugin_close_hint: "(Échap pour fermer)",
  ..EN
};

#[cfg(any(feature = "tui", test))]
impl PopupStrings {
  pub fn queue_empty(&self, key: &str) -> String {
    super::format_template(self.queue_empty_template, &[("key", key)])
  }

  pub fn queue_title(&self, key: &str) -> String {
    super::format_template(self.queue_title_template, &[("key", key)])
  }

  pub fn announcement_more(&self, url: &str) -> String {
    super::format_template(self.announcement_more_template, &[("url", url)])
  }

  pub fn recap_listens(&self, count: usize) -> String {
    super::format_template(
      self.recap_listens_template,
      &[("count", &count.to_string())],
    )
  }

  pub fn sort_field(&self, field: crate::core::sort::SortField) -> &'static str {
    use crate::core::sort::SortField;
    match field {
      SortField::Default => self.sort_default,
      SortField::Name => self.sort_name,
      SortField::Artist => self.sort_artist,
      SortField::Duration => self.sort_duration,
      SortField::Album => self.sort_album,
      SortField::ReleaseDate => self.sort_release_date,
    }
  }
}

#[cfg(test)]
mod tests {}
