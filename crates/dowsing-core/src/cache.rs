use crate::types::{CachedFileResult, NormalizationLevel};
use anyhow::Result;
use std::path::{Path, PathBuf};
use xxhash_rust::xxh3::xxh3_64;

/// Tool version string embedded in cache keys to invalidate on upgrades.
pub const TOOL_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Cache directory name (within the project root).
const CACHE_DIR: &str = ".dowsing-rod-cache";

/// Cache manager backed by simple per-file JSON files.
pub struct Cache {
    dir: PathBuf,
}

impl Cache {
    /// Open the cache in the given project root directory.
    /// Creates the cache directory if it doesn't exist.
    pub fn open(project_root: &Path) -> Result<Self> {
        let dir = project_root.join(CACHE_DIR);
        if !dir.exists() {
            std::fs::create_dir_all(&dir)?;
        }
        Ok(Self { dir })
    }

    /// Compute the cache key for a file given its content and configuration.
    ///
    /// Cache key components:
    /// - xxh3 hash of file content  (invalidates on file change)
    /// - tool version               (invalidates on tool upgrade)
    /// - normalization level        (invalidates on config change)
    fn cache_key(content: &[u8], normalization: NormalizationLevel) -> String {
        let content_hash = xxh3_64(content);
        format!("{content_hash:016x}-{TOOL_VERSION}-{normalization}")
    }

    /// Derive the cache file path for a source file.
    fn cache_path(&self, source_path: &Path) -> PathBuf {
        // Use a hash of the file path as the filename to avoid filesystem issues
        let path_str = source_path.display().to_string();
        let path_hash = xxh3_64(path_str.as_bytes());
        self.dir.join(format!("{path_hash:016x}.json"))
    }

    /// Try to load a cached result for a file.
    ///
    /// Returns None if:
    /// - cache entry doesn't exist
    /// - cache entry is stale (content/config changed)
    /// - cache entry is corrupted
    pub fn load(
        &self,
        source_path: &Path,
        content: &[u8],
        normalization: NormalizationLevel,
    ) -> Option<CachedFileResult> {
        let cache_path = self.cache_path(source_path);
        if !cache_path.exists() {
            return None;
        }

        let bytes = std::fs::read(&cache_path).ok()?;
        let cached: CachedFileResult = serde_json::from_slice(&bytes).ok()?;

        // Validate the cache key
        let expected_key = Self::cache_key(content, normalization);
        if cached.content_hash != expected_key {
            return None;
        }

        Some(cached)
    }

    /// Store a result in the cache.
    pub fn store(
        &self,
        source_path: &Path,
        _content: &[u8],
        _normalization: NormalizationLevel,
        result: &CachedFileResult,
    ) -> Result<()> {
        let cache_path = self.cache_path(source_path);
        let bytes = serde_json::to_vec(result)?;
        std::fs::write(cache_path, bytes)?;
        Ok(())
    }

    /// Build a CachedFileResult with the correct key.
    pub fn make_result(
        content: &[u8],
        normalization: NormalizationLevel,
        functions: Vec<crate::types::FunctionInfo>,
        normalized: Vec<crate::types::NormalizedFunction>,
        fingerprints: Vec<crate::types::Fingerprints>,
    ) -> CachedFileResult {
        CachedFileResult {
            content_hash: Self::cache_key(content, normalization),
            tool_version: TOOL_VERSION.to_string(),
            normalization,
            functions,
            normalized,
            fingerprints,
        }
    }

    /// Clear all cache entries.
    pub fn clear(&self) -> Result<usize> {
        let mut count = 0;
        if self.dir.exists() {
            for entry in std::fs::read_dir(&self.dir)? {
                let entry = entry?;
                if entry.path().extension().is_some_and(|e| e == "json") {
                    std::fs::remove_file(entry.path())?;
                    count += 1;
                }
            }
        }
        Ok(count)
    }

    /// Return the cache directory path.
    pub fn directory(&self) -> &Path {
        &self.dir
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_dummy_result(content: &[u8], norm: NormalizationLevel) -> CachedFileResult {
        Cache::make_result(content, norm, vec![], vec![], vec![])
    }

    #[test]
    fn test_cache_hit() {
        let dir = TempDir::new().unwrap();
        let cache = Cache::open(dir.path()).unwrap();
        let source = b"def foo(): pass";
        let path = dir.path().join("test.py");
        let result = make_dummy_result(source, NormalizationLevel::Balanced);

        cache
            .store(&path, source, NormalizationLevel::Balanced, &result)
            .unwrap();
        let loaded = cache.load(&path, source, NormalizationLevel::Balanced);
        assert!(loaded.is_some());
    }

    #[test]
    fn test_cache_miss_on_content_change() {
        let dir = TempDir::new().unwrap();
        let cache = Cache::open(dir.path()).unwrap();
        let path = dir.path().join("test.py");
        let original = b"def foo(): pass";
        let modified = b"def foo(): return 1";
        let result = make_dummy_result(original, NormalizationLevel::Balanced);

        cache
            .store(&path, original, NormalizationLevel::Balanced, &result)
            .unwrap();
        let loaded = cache.load(&path, modified, NormalizationLevel::Balanced);
        assert!(loaded.is_none());
    }

    #[test]
    fn test_cache_miss_on_normalization_change() {
        let dir = TempDir::new().unwrap();
        let cache = Cache::open(dir.path()).unwrap();
        let path = dir.path().join("test.py");
        let content = b"def foo(): pass";
        let result = make_dummy_result(content, NormalizationLevel::Balanced);

        cache
            .store(&path, content, NormalizationLevel::Balanced, &result)
            .unwrap();
        let loaded = cache.load(&path, content, NormalizationLevel::Strict);
        assert!(loaded.is_none());
    }

    #[test]
    fn test_cache_clear() {
        let dir = TempDir::new().unwrap();
        let cache = Cache::open(dir.path()).unwrap();
        let path = dir.path().join("test.py");
        let content = b"def foo(): pass";
        let result = make_dummy_result(content, NormalizationLevel::Balanced);
        cache
            .store(&path, content, NormalizationLevel::Balanced, &result)
            .unwrap();

        let cleared = cache.clear().unwrap();
        assert_eq!(cleared, 1);

        let loaded = cache.load(&path, content, NormalizationLevel::Balanced);
        assert!(loaded.is_none());
    }
}
