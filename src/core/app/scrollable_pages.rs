use super::*;

#[derive(Clone)]
pub struct ScrollableResultPages<T> {
  pub pages: Vec<T>,
}

impl<T> ScrollableResultPages<T> {
  pub fn new() -> ScrollableResultPages<T> {
    ScrollableResultPages { pages: vec![] }
  }
}

// Offset-keyed page caches are always kept sorted by `Paged::offset`, but the cache
// can be sparse, so visible-page identity is derived from the offset, never raw
// cache adjacency. There is no `DeserializeOwned` bound because `Paged` carries
// already-mapped domain items.
impl<T> ScrollableResultPages<Paged<T>> {}

#[cfg(test)]
mod tests {}
