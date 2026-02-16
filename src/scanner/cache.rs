use std::path::PathBuf;
use std::sync::LazyLock;
use std::time::{Duration, Instant};

use dashmap::DashMap;

const DEFAULT_TTL: Duration = Duration::from_secs(5);
const MAX_CACHE_SIZE: usize = 100;

#[derive(Debug, Clone)]
pub struct CachedEntry {
    pub name: String,
    pub is_dir: bool,
    pub is_hidden: bool,
    pub has_cmake: bool,
    pub extension: Option<String>,
}

#[derive(Debug, Clone)]
struct CachedDirectory {
    entries: Vec<CachedEntry>,
    cached_at: Instant,
}

impl CachedDirectory {
    fn is_expired(&self, ttl: Duration) -> bool {
        self.cached_at.elapsed() > ttl
    }
}

pub struct DirectoryCache {
    cache: DashMap<PathBuf, CachedDirectory>,
    ttl: Duration,
}

impl DirectoryCache {
    pub fn new() -> Self {
        Self {
            cache: DashMap::new(),
            ttl: DEFAULT_TTL,
        }
    }

    #[cfg(test)]
    #[allow(dead_code)]
    pub fn with_ttl(ttl: Duration) -> Self {
        Self {
            cache: DashMap::new(),
            ttl,
        }
    }

    pub fn get(&self, path: &PathBuf) -> Option<Vec<CachedEntry>> {
        let entry = self.cache.get(path)?;
        if entry.is_expired(self.ttl) {
            drop(entry);
            self.cache.remove(path);
            return None;
        }
        Some(entry.entries.clone())
    }

    pub fn insert(&self, path: PathBuf, entries: Vec<CachedEntry>) {
        if self.cache.len() >= MAX_CACHE_SIZE {
            self.evict_oldest();
        }
        self.cache.insert(
            path,
            CachedDirectory {
                entries,
                cached_at: Instant::now(),
            },
        );
    }

    pub fn invalidate(&self, path: &PathBuf) {
        self.cache.remove(path);
    }

    pub fn invalidate_subtree(&self, root: &PathBuf) {
        self.cache.retain(|path, _| !path.starts_with(root));
    }

    #[allow(dead_code)]
    pub fn clear(&self) {
        self.cache.clear();
    }

    #[allow(dead_code)]
    pub fn cleanup_expired(&self) {
        self.cache.retain(|_, entry| !entry.is_expired(self.ttl));
    }

    fn evict_oldest(&self) {
        let mut oldest: Option<(PathBuf, Instant)> = None;
        for entry in self.cache.iter() {
            let cached_at = entry.value().cached_at;
            match &oldest {
                None => oldest = Some((entry.key().clone(), cached_at)),
                Some((_, oldest_time)) if cached_at < *oldest_time => {
                    oldest = Some((entry.key().clone(), cached_at));
                }
                _ => {}
            }
        }

        if let Some((path, _)) = oldest {
            self.cache.remove(&path);
        }
    }

    #[allow(dead_code)]
    pub fn stats(&self) -> CacheStats {
        let mut expired = 0;
        let mut valid = 0;

        for entry in self.cache.iter() {
            if entry.value().is_expired(self.ttl) {
                expired += 1;
            } else {
                valid += 1;
            }
        }

        CacheStats {
            total: self.cache.len(),
            valid,
            expired,
        }
    }
}

impl Default for DirectoryCache {
    fn default() -> Self {
        Self::new()
    }
}

#[allow(dead_code)]
#[derive(Debug)]
pub struct CacheStats {
    pub total: usize,
    pub valid: usize,
    pub expired: usize,
}

pub static DIRECTORY_CACHE: LazyLock<DirectoryCache> = LazyLock::new(DirectoryCache::new);

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;

    #[test]
    fn test_cache_insert_and_get() {
        let cache = DirectoryCache::new();
        let path = PathBuf::from("/test/dir");
        let entries = vec![CachedEntry {
            name: "file.txt".to_string(),
            is_dir: false,
            is_hidden: false,
            has_cmake: false,
            extension: Some("txt".to_string()),
        }];

        cache.insert(path.clone(), entries.clone());

        let cached = cache.get(&path).unwrap();
        assert_eq!(cached.len(), 1);
        assert_eq!(cached[0].name, "file.txt");
    }

    #[test]
    fn test_cache_expiration() {
        let cache = DirectoryCache::with_ttl(Duration::from_millis(50));
        let path = PathBuf::from("/test/dir");
        let entries = vec![CachedEntry {
            name: "file.txt".to_string(),
            is_dir: false,
            is_hidden: false,
            has_cmake: false,
            extension: None,
        }];

        cache.insert(path.clone(), entries);

        // Should be cached
        assert!(cache.get(&path).is_some());

        // Wait for expiration
        sleep(Duration::from_millis(60));

        // Should be expired
        assert!(cache.get(&path).is_none());
    }

    #[test]
    fn test_cache_invalidate() {
        let cache = DirectoryCache::new();
        let path = PathBuf::from("/test/dir");
        let entries = vec![];

        cache.insert(path.clone(), entries);
        assert!(cache.get(&path).is_some());

        cache.invalidate(&path);
        assert!(cache.get(&path).is_none());
    }
}
