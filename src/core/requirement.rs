//! What a sidebar row, a key, or a setting needs before it can do anything:
//! a capability of the active source. The three terminal tables (sidebar, help menu, settings) carry a
//! [`Requirement`] per row and filter through [`availability`], so the rows
//! a frontend offers and the keys that work cannot disagree.

use crate::core::source::Source;

/// A capability of the active source, one per `Source::supports_*` predicate.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Capability {
  Search,
}

impl Capability {
  pub fn supported_by(self, source: Source) -> bool {
    match self {
      Capability::Search => source.supports_search(),
    }
  }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Requirement {
  None,
  /// The active source must have the capability.
  Capability(Capability),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Availability {
  Available,
  /// The active source cannot do it.
  NotForSource(Source),
}

impl Availability {
  pub fn is_available(self) -> bool {
    self == Availability::Available
  }

  /// The suffix in the selected language, including the localized source name.
  pub fn localized_hint(self, strings: &crate::core::i18n::Strings) -> Option<String> {
    match self {
      Availability::Available => None,
      Availability::NotForSource(source) => Some(
        strings
          .help
          .not_for_source(strings.sidebar.source_label(source)),
      ),
    }
  }
}

pub fn availability(requirement: Requirement, source: Source) -> Availability {
  match requirement {
    Requirement::None => Availability::Available,
    Requirement::Capability(capability) => {
      if !capability.supported_by(source) {
        Availability::NotForSource(source)
      } else {
        Availability::Available
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn no_requirement_is_always_available() {
    for source in Source::ALL {
      assert!(availability(Requirement::None, source).is_available());
    }
  }
}
