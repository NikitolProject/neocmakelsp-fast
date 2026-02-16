use std::path::{Path, PathBuf};
use std::sync::mpsc;

use ignore::WalkBuilder;

use super::cache::{CachedEntry, DIRECTORY_CACHE};

#[derive(Debug, Clone, Default)]
pub struct ScanOptions {
    pub dirs_only: bool,
    pub extensions: Option<Vec<String>>,
    pub include_hidden: bool,
    pub check_cmake: bool,
    pub max_depth: Option<usize>,
    pub respect_gitignore: bool,
}

impl ScanOptions {
    pub fn for_subdirectory() -> Self {
        Self {
            dirs_only: true,
            extensions: None,
            include_hidden: false,
            check_cmake: true,
            max_depth: Some(1),
            respect_gitignore: true,
        }
    }

    pub fn for_include() -> Self {
        Self {
            dirs_only: false,
            extensions: Some(vec!["cmake".to_string()]),
            include_hidden: false,
            check_cmake: false,
            max_depth: Some(1),
            respect_gitignore: true,
        }
    }

    pub fn for_source_files() -> Self {
        Self {
            dirs_only: false,
            extensions: Some(vec![
                "c".to_string(),
                "cc".to_string(),
                "cpp".to_string(),
                "cxx".to_string(),
                "c++".to_string(),
                "h".to_string(),
                "hh".to_string(),
                "hpp".to_string(),
                "hxx".to_string(),
                "h++".to_string(),
                "m".to_string(),
                "mm".to_string(),
                "cu".to_string(),
                "cuh".to_string(),
                "asm".to_string(),
                "s".to_string(),
                "f".to_string(),
                "f90".to_string(),
                "f95".to_string(),
                "for".to_string(),
                "rc".to_string(),
            ]),
            include_hidden: false,
            check_cmake: false,
            max_depth: Some(1),
            respect_gitignore: true,
        }
    }

    pub fn for_any_file() -> Self {
        Self {
            dirs_only: false,
            extensions: None,
            include_hidden: false,
            check_cmake: false,
            max_depth: Some(1),
            respect_gitignore: true,
        }
    }

    pub fn for_directory() -> Self {
        Self {
            dirs_only: true,
            extensions: None,
            include_hidden: false,
            check_cmake: false,
            max_depth: Some(1),
            respect_gitignore: true,
        }
    }
}

pub fn scan_directory<P: AsRef<Path>>(dir: P, options: &ScanOptions) -> Vec<CachedEntry> {
    let dir = dir.as_ref();
    let dir_path = dir.to_path_buf();

    if let Some(cached) = DIRECTORY_CACHE.get(&dir_path) {
        return filter_entries(cached, options);
    }

    let entries = scan_directory_internal(dir, options);
    let full_entries = scan_directory_full(dir);
    DIRECTORY_CACHE.insert(dir_path, full_entries);
    entries
}

fn scan_directory_internal<P: AsRef<Path>>(dir: P, options: &ScanOptions) -> Vec<CachedEntry> {
    let dir = dir.as_ref();
    if !dir.exists() || !dir.is_dir() {
        return Vec::new();
    }

    let mut entries = Vec::new();
    let walker = WalkBuilder::new(dir)
        .max_depth(options.max_depth)
        .hidden(!options.include_hidden)
        .git_ignore(options.respect_gitignore)
        .git_global(options.respect_gitignore)
        .git_exclude(options.respect_gitignore)
        .build();

    for entry in walker.flatten() {
        if entry.path() == dir {
            continue;
        }

        let path = entry.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };

        let is_dir = path.is_dir();
        let is_hidden = name.starts_with('.');

        if is_hidden && !options.include_hidden {
            continue;
        }

        if options.dirs_only && !is_dir {
            continue;
        }

        let extension = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|s| s.to_string());
        if let Some(ref allowed_exts) = options.extensions
            && !is_dir
        {
            match &extension {
                Some(ext) if allowed_exts.contains(ext) => {}
                _ => continue,
            }
        }

        let has_cmake = if is_dir && options.check_cmake {
            path.join("CMakeLists.txt").exists()
        } else {
            false
        };

        entries.push(CachedEntry {
            name: name.to_string(),
            is_dir,
            is_hidden,
            has_cmake,
            extension,
        });
    }

    entries
}

fn scan_directory_full<P: AsRef<Path>>(dir: P) -> Vec<CachedEntry> {
    let dir = dir.as_ref();
    if !dir.exists() || !dir.is_dir() {
        return Vec::new();
    }

    let mut entries = Vec::new();
    if let Ok(read_dir) = std::fs::read_dir(dir) {
        for entry in read_dir.flatten() {
            let path = entry.path();
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };

            let is_dir = path.is_dir();
            let is_hidden = name.starts_with('.');
            let extension = path
                .extension()
                .and_then(|e| e.to_str())
                .map(|s| s.to_string());
            let has_cmake = if is_dir {
                path.join("CMakeLists.txt").exists()
            } else {
                false
            };

            entries.push(CachedEntry {
                name: name.to_string(),
                is_dir,
                is_hidden,
                has_cmake,
                extension,
            });
        }
    }

    entries
}

fn filter_entries(entries: Vec<CachedEntry>, options: &ScanOptions) -> Vec<CachedEntry> {
    entries
        .into_iter()
        .filter(|entry| {
            if entry.is_hidden && !options.include_hidden {
                return false;
            }

            if options.dirs_only && !entry.is_dir {
                return false;
            }

            if let Some(ref allowed_exts) = options.extensions
                && !entry.is_dir
            {
                match &entry.extension {
                    Some(ext) if allowed_exts.contains(ext) => {}
                    _ => return false,
                }
            }

            true
        })
        .collect()
}

#[allow(dead_code)]
pub fn scan_directory_recursive<P: AsRef<Path>>(
    dir: P,
    options: &ScanOptions,
) -> Vec<(PathBuf, CachedEntry)> {
    let dir = dir.as_ref();
    if !dir.exists() || !dir.is_dir() {
        return Vec::new();
    }

    let (tx, rx) = mpsc::channel();
    let walker = WalkBuilder::new(dir)
        .max_depth(options.max_depth)
        .hidden(!options.include_hidden)
        .git_ignore(options.respect_gitignore)
        .threads(num_cpus::get().min(4)) // Limit threads
        .build_parallel();

    walker.run(|| {
        let tx = tx.clone();
        let options = options.clone();

        Box::new(move |entry| {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => return ignore::WalkState::Continue,
            };

            let path = entry.path();
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                return ignore::WalkState::Continue;
            };

            let is_dir = path.is_dir();
            let is_hidden = name.starts_with('.');

            if is_hidden && !options.include_hidden {
                return ignore::WalkState::Continue;
            }

            if options.dirs_only && !is_dir {
                return ignore::WalkState::Continue;
            }

            let extension = path
                .extension()
                .and_then(|e| e.to_str())
                .map(|s| s.to_string());

            if let Some(ref allowed_exts) = options.extensions
                && !is_dir
            {
                match &extension {
                    Some(ext) if allowed_exts.contains(ext) => {}
                    _ => return ignore::WalkState::Continue,
                }
            }

            let has_cmake = if is_dir && options.check_cmake {
                path.join("CMakeLists.txt").exists()
            } else {
                false
            };

            let cached_entry = CachedEntry {
                name: name.to_string(),
                is_dir,
                is_hidden,
                has_cmake,
                extension,
            };

            let _ = tx.send((path.to_path_buf(), cached_entry));

            ignore::WalkState::Continue
        })
    });

    drop(tx);
    rx.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};
    use tempfile::tempdir;

    #[test]
    fn test_scan_directory_basic() {
        let dir = tempdir().unwrap();
        File::create(dir.path().join("file.txt")).unwrap();
        fs::create_dir(dir.path().join("subdir")).unwrap();

        let entries = scan_directory(dir.path(), &ScanOptions::default());
        assert_eq!(entries.len(), 2);
    }

    #[test]
    fn test_scan_directory_dirs_only() {
        let dir = tempdir().unwrap();
        File::create(dir.path().join("file.txt")).unwrap();
        fs::create_dir(dir.path().join("subdir")).unwrap();

        let entries = scan_directory(dir.path(), &ScanOptions::for_subdirectory());
        assert_eq!(entries.len(), 1);
        assert!(entries[0].is_dir);
    }

    #[test]
    fn test_scan_directory_cmake_check() {
        let dir = tempdir().unwrap();
        let subdir = dir.path().join("with_cmake");
        fs::create_dir(&subdir).unwrap();
        File::create(subdir.join("CMakeLists.txt")).unwrap();

        let subdir2 = dir.path().join("without_cmake");
        fs::create_dir(&subdir2).unwrap();

        let entries = scan_directory(dir.path(), &ScanOptions::for_subdirectory());
        assert_eq!(entries.len(), 2);

        let with_cmake = entries.iter().find(|e| e.name == "with_cmake").unwrap();
        assert!(with_cmake.has_cmake);

        let without_cmake = entries.iter().find(|e| e.name == "without_cmake").unwrap();
        assert!(!without_cmake.has_cmake);
    }

    #[test]
    fn test_scan_directory_extension_filter() {
        let dir = tempdir().unwrap();
        File::create(dir.path().join("source.cpp")).unwrap();
        File::create(dir.path().join("header.hpp")).unwrap();
        File::create(dir.path().join("readme.txt")).unwrap();

        let entries = scan_directory(dir.path(), &ScanOptions::for_source_files());
        assert_eq!(entries.len(), 2);
    }
}
