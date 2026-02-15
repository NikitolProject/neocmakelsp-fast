//! Path completion module with caching support.
//!
//! This module provides path completions for various CMake commands,
//! using the scanner module for cached directory scanning.

use std::path::Path;

use tower_lsp::lsp_types::{
    CompletionItem, CompletionItemKind, CompletionTextEdit, Position, Range, TextEdit,
};

use crate::scanner::{scan_directory, CachedEntry, ScanOptions};

/// Result of extracting partial path - includes the path and its start position
#[derive(Debug, Clone)]
pub struct PartialPathInfo {
    pub path: String,
    pub start_character: u32,
}

/// Check if the partial input looks like a file path.
/// Returns true if input starts with '.', '/', '~', or contains '/'.
pub fn looks_like_path(partial: &str) -> bool {
    if partial.is_empty() {
        return false;
    }

    // Starts with path-like characters
    if partial.starts_with('.')
        || partial.starts_with('/')
        || partial.starts_with('~')
    {
        return true;
    }

    // Contains path separator
    if partial.contains('/') {
        return true;
    }

    // Looks like a file with extension (e.g., "main.cpp", "config.cmake")
    if let Some(dot_pos) = partial.rfind('.') {
        let ext = &partial[dot_pos + 1..];
        // Check if it has a reasonable extension (1-10 chars, alphanumeric)
        if !ext.is_empty() && ext.len() <= 10 && ext.chars().all(|c| c.is_ascii_alphanumeric()) {
            return true;
        }
    }

    false
}

/// Extract the partial path input from the current line at the given position.
/// Returns the partial path and the character position where it starts.
pub fn extract_partial_path(source: &str, line: u32, character: u32) -> PartialPathInfo {
    let lines: Vec<&str> = source.lines().collect();
    if (line as usize) >= lines.len() {
        return PartialPathInfo {
            path: String::new(),
            start_character: character,
        };
    }

    let current_line = lines[line as usize];
    let char_pos = character as usize;

    if char_pos > current_line.len() {
        return PartialPathInfo {
            path: String::new(),
            start_character: character,
        };
    }

    // Find the start of the argument (after opening quote or paren)
    let before_cursor = &current_line[..char_pos];

    // Look for the start of the path argument
    // Could be after: ( " ' or whitespace
    let start_pos = before_cursor
        .rfind(|c: char| c == '(' || c == '"' || c == '\'' || c == ' ' || c == '\t')
        .map(|pos| pos + 1)
        .unwrap_or(0);

    let partial = &before_cursor[start_pos..];

    // Remove any quotes
    let path = partial.trim_matches(|c| c == '"' || c == '\'').to_string();

    PartialPathInfo {
        path,
        start_character: start_pos as u32,
    }
}

/// Determine search directory and prefix from partial input
fn resolve_search_path<P: AsRef<Path>>(
    base_dir: P,
    partial_input: &str,
) -> (std::path::PathBuf, String) {
    let base_dir = base_dir.as_ref();

    if partial_input.is_empty() {
        (base_dir.to_path_buf(), String::new())
    } else if partial_input.ends_with('/') {
        (base_dir.join(partial_input), partial_input.to_string())
    } else {
        let path = Path::new(partial_input);
        if let Some(parent) = path.parent() {
            if parent.as_os_str().is_empty() {
                (base_dir.to_path_buf(), String::new())
            } else {
                let parent_str = parent.to_string_lossy();
                (base_dir.join(parent), format!("{}/", parent_str))
            }
        } else {
            (base_dir.to_path_buf(), String::new())
        }
    }
}

/// Convert cached entries to completion items
fn entries_to_completions(
    entries: Vec<CachedEntry>,
    prefix: &str,
    replace_range: Range,
    options: &CompletionOptions,
) -> Vec<CompletionItem> {
    entries
        .into_iter()
        .map(|entry| {
            let label = if entry.is_dir {
                format!("{}/", entry.name)
            } else {
                entry.name.clone()
            };

            let new_text = format!("{}{}", prefix, entry.name);
            let filter_text = new_text.clone();

            // Sort: directories with CMakeLists.txt first, then files, then other dirs
            let sort_text = if entry.is_dir {
                if entry.has_cmake {
                    format!("!0_{}", entry.name)
                } else {
                    format!("!2_{}", entry.name)
                }
            } else {
                format!("!1_{}", entry.name)
            };

            let kind = if entry.is_dir {
                CompletionItemKind::FOLDER
            } else {
                CompletionItemKind::FILE
            };

            // Show ✓ for directories with CMakeLists.txt
            let detail = if options.show_cmake_marker && entry.is_dir && entry.has_cmake {
                Some("✓".to_string())
            } else {
                None
            };

            CompletionItem {
                label,
                kind: Some(kind),
                detail,
                documentation: None,
                sort_text: Some(sort_text),
                filter_text: Some(filter_text),
                text_edit: Some(CompletionTextEdit::Edit(TextEdit {
                    range: replace_range,
                    new_text,
                })),
                ..Default::default()
            }
        })
        .collect()
}

/// Options for completion generation
struct CompletionOptions {
    show_cmake_marker: bool,
}

impl Default for CompletionOptions {
    fn default() -> Self {
        Self {
            show_cmake_marker: false,
        }
    }
}

/// Get path completions for add_subdirectory command.
/// Returns directories that contain CMakeLists.txt relative to the current file.
pub fn get_subdirectory_completions<P: AsRef<Path>>(
    current_file: P,
    partial_info: &PartialPathInfo,
    line: u32,
    character: u32,
) -> Vec<CompletionItem> {
    let current_file = current_file.as_ref();
    let base_dir = current_file.parent().unwrap_or(Path::new("."));

    let (search_dir, prefix) = resolve_search_path(base_dir, &partial_info.path);

    let replace_range = Range {
        start: Position {
            line,
            character: partial_info.start_character,
        },
        end: Position { line, character },
    };

    let entries = scan_directory(&search_dir, &ScanOptions::for_subdirectory());

    entries_to_completions(
        entries,
        &prefix,
        replace_range,
        &CompletionOptions {
            show_cmake_marker: true,
        },
    )
}

/// Get path completions for include command.
/// Returns .cmake files relative to the current file.
pub fn get_include_path_completions<P: AsRef<Path>>(
    current_file: P,
    partial_info: &PartialPathInfo,
    line: u32,
    character: u32,
) -> Vec<CompletionItem> {
    let current_file = current_file.as_ref();
    let base_dir = current_file.parent().unwrap_or(Path::new("."));

    let (search_dir, prefix) = resolve_search_path(base_dir, &partial_info.path);

    let replace_range = Range {
        start: Position {
            line,
            character: partial_info.start_character,
        },
        end: Position { line, character },
    };

    let entries = scan_directory(&search_dir, &ScanOptions::for_include());

    entries_to_completions(entries, &prefix, replace_range, &CompletionOptions::default())
}

/// Get path completions for source file commands (add_executable, add_library, target_sources).
/// Returns source files (.c, .cpp, .h, etc.) and directories relative to the current file.
pub fn get_source_file_completions<P: AsRef<Path>>(
    current_file: P,
    partial_info: &PartialPathInfo,
    line: u32,
    character: u32,
) -> Vec<CompletionItem> {
    let current_file = current_file.as_ref();
    let base_dir = current_file.parent().unwrap_or(Path::new("."));

    let (search_dir, prefix) = resolve_search_path(base_dir, &partial_info.path);

    let replace_range = Range {
        start: Position {
            line,
            character: partial_info.start_character,
        },
        end: Position { line, character },
    };

    let entries = scan_directory(&search_dir, &ScanOptions::for_source_files());

    entries_to_completions(entries, &prefix, replace_range, &CompletionOptions::default())
}

/// Get path completions for any file commands (file(), configure_file, install(FILES), etc.).
/// Returns all files and directories relative to the current file.
pub fn get_any_file_completions<P: AsRef<Path>>(
    current_file: P,
    partial_info: &PartialPathInfo,
    line: u32,
    character: u32,
) -> Vec<CompletionItem> {
    let current_file = current_file.as_ref();
    let base_dir = current_file.parent().unwrap_or(Path::new("."));

    let (search_dir, prefix) = resolve_search_path(base_dir, &partial_info.path);

    let replace_range = Range {
        start: Position {
            line,
            character: partial_info.start_character,
        },
        end: Position { line, character },
    };

    let entries = scan_directory(&search_dir, &ScanOptions::for_any_file());

    entries_to_completions(entries, &prefix, replace_range, &CompletionOptions::default())
}

/// Get path completions for directory commands (install(DIRECTORY)).
/// Returns only directories relative to the current file.
pub fn get_directory_completions<P: AsRef<Path>>(
    current_file: P,
    partial_info: &PartialPathInfo,
    line: u32,
    character: u32,
) -> Vec<CompletionItem> {
    let current_file = current_file.as_ref();
    let base_dir = current_file.parent().unwrap_or(Path::new("."));

    let (search_dir, prefix) = resolve_search_path(base_dir, &partial_info.path);

    let replace_range = Range {
        start: Position {
            line,
            character: partial_info.start_character,
        },
        end: Position { line, character },
    };

    let entries = scan_directory(&search_dir, &ScanOptions::for_directory());

    entries_to_completions(entries, &prefix, replace_range, &CompletionOptions::default())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};
    use tempfile::tempdir;

    #[test]
    fn test_looks_like_path() {
        // Should return true for path-like inputs
        assert!(looks_like_path("./src"));
        assert!(looks_like_path("../lib"));
        assert!(looks_like_path("/usr/local"));
        assert!(looks_like_path("~/projects"));
        assert!(looks_like_path("src/main.cpp"));
        assert!(looks_like_path("main.cpp"));
        assert!(looks_like_path("config.cmake"));
        assert!(looks_like_path("CMakeLists.txt"));

        // Should return false for non-path inputs
        assert!(!looks_like_path(""));
        assert!(!looks_like_path("WIN32"));
        assert!(!looks_like_path("MACOSX_BUNDLE"));
        assert!(!looks_like_path("PUBLIC"));
        assert!(!looks_like_path("PRIVATE"));
        assert!(!looks_like_path("MyTarget"));
        assert!(!looks_like_path("${VAR}"));
    }

    #[test]
    fn test_extract_partial_path() {
        let source = r#"
add_subdirectory(src/)
include("cmake/mo")
"#;
        // "add_subdirectory(src/)" - position 21 is right after the '/'
        let info1 = extract_partial_path(source, 1, 21);
        assert_eq!(info1.path, "src/");

        // "include(\"cmake/mo\")" - position 17 is right after 'mo'
        let info2 = extract_partial_path(source, 2, 17);
        assert_eq!(info2.path, "cmake/mo");
    }

    #[test]
    fn test_subdirectory_completions() {
        let dir = tempdir().unwrap();
        let cmake_file = dir.path().join("CMakeLists.txt");
        File::create(&cmake_file).unwrap();

        // Create subdirectories
        let src_dir = dir.path().join("src");
        fs::create_dir(&src_dir).unwrap();
        File::create(src_dir.join("CMakeLists.txt")).unwrap();

        let tests_dir = dir.path().join("tests");
        fs::create_dir(&tests_dir).unwrap();
        // No CMakeLists.txt in tests

        let partial_info = PartialPathInfo {
            path: String::new(),
            start_character: 17, // after "add_subdirectory("
        };
        let completions = get_subdirectory_completions(&cmake_file, &partial_info, 0, 17);
        assert!(!completions.is_empty());

        // src should be prioritized (has CMakeLists.txt)
        let src_item = completions.iter().find(|c| c.label == "src/").unwrap();
        assert!(src_item.sort_text.as_ref().unwrap().starts_with("!0_"));
        assert_eq!(src_item.detail, Some("✓".to_string()));

        // tests should be lower priority (no CMakeLists.txt)
        let tests_item = completions.iter().find(|c| c.label == "tests/").unwrap();
        assert!(tests_item.sort_text.as_ref().unwrap().starts_with("!2_"));
        assert_eq!(tests_item.detail, None);
    }

    #[test]
    fn test_source_file_completions() {
        let dir = tempdir().unwrap();
        let cmake_file = dir.path().join("CMakeLists.txt");
        File::create(&cmake_file).unwrap();

        // Create source files
        let src_dir = dir.path().join("src");
        fs::create_dir(&src_dir).unwrap();
        File::create(src_dir.join("main.cpp")).unwrap();
        File::create(src_dir.join("util.c")).unwrap();
        File::create(src_dir.join("header.h")).unwrap();
        File::create(src_dir.join("readme.txt")).unwrap(); // Should not appear

        let partial_info = PartialPathInfo {
            path: "src/".to_string(),
            start_character: 16,
        };
        let completions = get_source_file_completions(&cmake_file, &partial_info, 0, 20);

        // Should find source files but not readme.txt
        assert!(completions.iter().any(|c| c.label == "main.cpp"));
        assert!(completions.iter().any(|c| c.label == "util.c"));
        assert!(completions.iter().any(|c| c.label == "header.h"));
        assert!(!completions.iter().any(|c| c.label == "readme.txt"));
    }

    #[test]
    fn test_any_file_completions() {
        let dir = tempdir().unwrap();
        let cmake_file = dir.path().join("CMakeLists.txt");
        File::create(&cmake_file).unwrap();

        // Create various files
        File::create(dir.path().join("config.txt")).unwrap();
        File::create(dir.path().join("data.json")).unwrap();
        File::create(dir.path().join("script.sh")).unwrap();

        let partial_info = PartialPathInfo {
            path: String::new(),
            start_character: 10,
        };
        let completions = get_any_file_completions(&cmake_file, &partial_info, 0, 10);

        // Should find all files
        assert!(completions.iter().any(|c| c.label == "config.txt"));
        assert!(completions.iter().any(|c| c.label == "data.json"));
        assert!(completions.iter().any(|c| c.label == "script.sh"));
        // CMakeLists.txt should also be included
        assert!(completions.iter().any(|c| c.label == "CMakeLists.txt"));
    }

    #[test]
    fn test_include_completions() {
        let dir = tempdir().unwrap();
        let cmake_file = dir.path().join("CMakeLists.txt");
        File::create(&cmake_file).unwrap();

        // Create cmake directory with modules
        let cmake_dir = dir.path().join("cmake");
        fs::create_dir(&cmake_dir).unwrap();
        File::create(cmake_dir.join("FindFoo.cmake")).unwrap();
        File::create(cmake_dir.join("utils.cmake")).unwrap();

        let partial_info = PartialPathInfo {
            path: "cmake/".to_string(),
            start_character: 9, // after "include(\""
        };
        let completions = get_include_path_completions(&cmake_file, &partial_info, 0, 15);
        assert!(!completions.is_empty());

        // Should find .cmake files
        assert!(completions.iter().any(|c| c.label == "FindFoo.cmake"));
        assert!(completions.iter().any(|c| c.label == "utils.cmake"));

        // Check that filter_text includes the prefix for proper filtering
        let foo_item = completions
            .iter()
            .find(|c| c.label == "FindFoo.cmake")
            .unwrap();
        assert_eq!(
            foo_item.filter_text.as_ref().unwrap(),
            "cmake/FindFoo.cmake"
        );
    }

    #[test]
    fn test_extract_partial_path_incomplete_command() {
        // Test the scenario: add_executable(my_app ./
        // Cursor at column 24 (after the /)
        let source = "add_executable(my_app ./";
        let info = extract_partial_path(source, 0, 24);

        // The partial path should be "./" (starting after the space at position 21)
        assert_eq!(info.path, "./");
        assert_eq!(info.start_character, 22); // After the space

        // Verify looks_like_path recognizes it
        assert!(looks_like_path(&info.path));
    }

    #[test]
    fn test_extract_partial_path_dot_only() {
        // Test when user has typed just "."
        let source = "add_executable(my_app .";
        let info = extract_partial_path(source, 0, 23);

        assert_eq!(info.path, ".");
        assert!(looks_like_path(&info.path));
    }
}
