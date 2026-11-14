use super::*;

#[derive(Clone)]
pub struct ScrollableResultPages<T> {
  pub index: usize,
  pub pages: Vec<T>,
  /// Set when the cached pages were mutated underneath the reader (e.g. a
  /// track was added to or removed from an open playlist). `is_complete`
  /// honours it, so a search or sort walks Spotify again instead of reading
  /// pages that no longer match what is on the server. Cleared by `clear()`.
  pub stale: bool,
}

impl<T> ScrollableResultPages<T> {
  pub fn new() -> ScrollableResultPages<T> {
    ScrollableResultPages {
      index: 0,
      pages: vec![],
      stale: false,
    }
  }

  pub fn get_results(&self, at_index: Option<usize>) -> Option<&T> {
    self.pages.get(at_index.unwrap_or(self.index))
  }
}

// Offset-keyed page caches are always kept sorted by `Paged::offset`, but the cache
// can be sparse, so visible-page identity is derived from the offset, never raw
// cache adjacency. There is no `DeserializeOwned` bound because `Paged` carries
// already-mapped domain items.
impl<T> ScrollableResultPages<Paged<T>> {
  pub fn page_index_for_offset(&self, offset: u32) -> Option<usize> {
    self
      .pages
      .binary_search_by_key(&offset, |page| page.offset)
      .ok()
  }
}

#[cfg(test)]
mod tests {}
