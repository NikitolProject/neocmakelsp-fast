use std::collections::HashMap;
use std::iter::zip;
use std::process::Command;
use std::sync::LazyLock;

use anyhow::Result;
use tower_lsp::lsp_types::{CompletionItem, CompletionItemKind, Documentation, InsertTextFormat};

use crate::languageserver::to_use_snippet;

fn gen_builtin_commands(raw_info: &str) -> Result<Vec<CompletionItem>> {
    let re = regex::Regex::new(r"[a-zA-z]+\n-+").unwrap();
    let keys: Vec<_> = re
        .find_iter(raw_info)
        .map(|message| {
            let temp: Vec<&str> = message.as_str().split('\n').collect();
            temp[0]
        })
        .collect();
    let contents: Vec<_> = re.split(raw_info).collect();
    let contents = &contents[1..].to_vec();

    let mut completes = HashMap::new();
    for (key, content) in keys.iter().zip(contents) {
        let small_key = key.to_lowercase();
        let big_key = key.to_uppercase();
        completes.insert(small_key, content.to_string());
        completes.insert(big_key, content.to_string());
    }
    #[cfg(unix)]
    {
        completes.insert(
            "pkg_check_modules".to_string(),
            "please findpackage PkgConfig first".to_string(),
        );
        completes.insert(
            "PKG_CHECK_MODULES".to_string(),
            "please findpackage PkgConfig first".to_string(),
        );
    }

    let client_support_snippet = to_use_snippet();

    Ok(completes
        .iter()
        .map(|(akey, message)| {
            // Simple snippet: just add parentheses with cursor inside
            let (insert_text, insert_text_format) = if client_support_snippet
                && akey.chars().all(|c| c.is_ascii_lowercase() || c == '_')
            {
                (
                    Some(format!("{}($0)", akey)),
                    Some(InsertTextFormat::SNIPPET),
                )
            } else {
                (Some(akey.to_string()), Some(InsertTextFormat::PLAIN_TEXT))
            };

            // Prioritize lowercase commands (sort_text "0_" prefix) over uppercase ("1_" prefix)
            let is_lowercase = akey.chars().all(|c| c.is_ascii_lowercase() || c == '_');
            let sort_text = if is_lowercase {
                format!("0_{akey}")
            } else {
                format!("1_{akey}")
            };

            CompletionItem {
                label: akey.to_string(),
                kind: Some(CompletionItemKind::FUNCTION),
                detail: Some("Function".to_string()),
                documentation: Some(Documentation::String(message.trim().to_string())),
                insert_text,
                insert_text_format,
                sort_text: Some(sort_text),
                filter_text: Some(akey.to_lowercase()),
                ..Default::default()
            }
        })
        .collect())
}

fn gen_builtin_variables(raw_info: &str) -> Result<Vec<CompletionItem>> {
    let re = regex::Regex::new(r"[z-zA-z]+\n-+").unwrap();
    let key: Vec<_> = re
        .find_iter(raw_info)
        .map(|message| {
            let temp: Vec<&str> = message.as_str().split('\n').collect();
            temp[0]
        })
        .collect();
    let content: Vec<_> = re.split(raw_info).collect();
    let context = &content[1..];
    Ok(zip(key, context)
        .map(|(akey, message)| CompletionItem {
            label: akey.to_string(),
            kind: Some(CompletionItemKind::VARIABLE),
            detail: Some("Variable".to_string()),
            documentation: Some(Documentation::String(message.trim().to_string())),
            ..Default::default()
        })
        .collect())
}

fn gen_builtin_modules(raw_info: &str) -> Result<Vec<CompletionItem>> {
    let re = regex::Regex::new(r"[z-zA-z]+\n-+").unwrap();
    let key: Vec<_> = re
        .find_iter(raw_info)
        .map(|message| {
            let temp: Vec<&str> = message.as_str().split('\n').collect();
            temp[0]
        })
        .collect();
    let content: Vec<_> = re.split(raw_info).collect();
    let context = &content[1..];
    Ok(zip(key, context)
        .map(|(akey, message)| CompletionItem {
            label: akey.to_string(),
            kind: Some(CompletionItemKind::MODULE),
            detail: Some("Module".to_string()),
            documentation: Some(Documentation::String(message.trim().to_string())),
            ..Default::default()
        })
        .collect())
}

/// CMake builtin commands
pub static BUILTIN_COMMAND: LazyLock<Result<Vec<CompletionItem>>> = LazyLock::new(|| {
    let output = Command::new("cmake")
        .arg("--help-commands")
        .output()?
        .stdout;
    let temp = String::from_utf8_lossy(&output);
    gen_builtin_commands(&temp)
});

/// cmake builtin vars
pub static BUILTIN_VARIABLE: LazyLock<Result<Vec<CompletionItem>>> = LazyLock::new(|| {
    let output = Command::new("cmake")
        .arg("--help-variables")
        .output()?
        .stdout;
    let temp = String::from_utf8_lossy(&output);
    gen_builtin_variables(&temp)
});

/// Cmake builtin modules
pub static BUILTIN_MODULE: LazyLock<Result<Vec<CompletionItem>>> = LazyLock::new(|| {
    let output = Command::new("cmake").arg("--help-modules").output()?.stdout;
    let temp = String::from_utf8_lossy(&output);
    gen_builtin_modules(&temp)
});

#[cfg(test)]
mod tests {
    use std::iter::zip;

    use super::*;
    use crate::complete::builtin::{gen_builtin_modules, gen_builtin_variables};

    #[test]
    fn test_regex() {
        let re = regex::Regex::new(r"-+").unwrap();
        assert!(re.is_match("---------"));
        assert!(re.is_match("-------------------"));
        let temp = "javascrpt---------it is";
        let splits: Vec<_> = re.split(temp).collect();
        let aftersplit = vec!["javascrpt", "it is"];
        for (split, after) in zip(splits, aftersplit) {
            assert_eq!(split, after);
        }
    }

    #[test]
    fn test_cmake_command_builtin() {
        // NOTE: In case the command fails, ignore test
        let output = include_str!("../../assets_for_test/cmake_help_commands.txt");

        let output = gen_builtin_commands(output);

        assert!(output.is_ok());
    }

    #[test]
    fn test_cmake_variables_builtin() {
        // NOTE: In case the command fails, ignore test
        let output = include_str!("../../assets_for_test/cmake_help_variables.txt");

        let output = gen_builtin_variables(output);

        assert!(output.is_ok());
    }

    #[test]
    fn test_cmake_modules_builtin() {
        // NOTE: In case the command fails, ignore test
        let output = include_str!("../../assets_for_test/cmake_help_commands.txt");

        let output = gen_builtin_modules(output);

        assert!(output.is_ok());
    }
}
