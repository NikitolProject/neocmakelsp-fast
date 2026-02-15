use std::collections::HashMap;
use std::process::Command;
use std::sync::LazyLock;

use tower_lsp::lsp_types::{
    Documentation, MarkupContent, MarkupKind, ParameterInformation, ParameterLabel, Position,
    SignatureHelp, SignatureInformation,
};
use tree_sitter::{Node, Point};

use crate::consts::TREESITTER_CMAKE_LANGUAGE;
use crate::utils::treehelper::ToPoint;
use crate::CMakeNodeKinds;

/// Parsed signature information for a CMake command
#[derive(Debug, Clone)]
pub struct CMakeSignature {
    pub label: String,
    pub documentation: String,
    pub parameters: Vec<String>,
}

/// Extract signatures from cmake --help-commands output
fn parse_signatures_from_help(raw_info: &str) -> HashMap<String, Vec<CMakeSignature>> {
    let mut signatures: HashMap<String, Vec<CMakeSignature>> = HashMap::new();

    // Split by command headers (command_name followed by dashes)
    let re = regex::Regex::new(r"([a-zA-Z_][a-zA-Z0-9_]*)\n-+").unwrap();
    let keys: Vec<_> = re
        .find_iter(raw_info)
        .map(|message| {
            let temp: Vec<&str> = message.as_str().split('\n').collect();
            temp[0].to_lowercase()
        })
        .collect();
    let contents: Vec<_> = re.split(raw_info).collect();
    let contents = &contents[1..];

    for (key, content) in keys.iter().zip(contents) {
        // Find all signature patterns like: command_name(<args>)
        let sig_pattern = format!(r"(?m)^\s*{}\s*\(([^)]*)\)", regex::escape(key));
        let sig_re = regex::Regex::new(&sig_pattern).unwrap_or_else(|_| {
            regex::Regex::new(r"^\s*\w+\s*\(([^)]*)\)").unwrap()
        });

        let mut cmd_signatures = Vec::new();

        for caps in sig_re.captures_iter(content) {
            if let Some(args_match) = caps.get(1) {
                let args_str = args_match.as_str().trim();
                let full_sig = format!("{}({})", key, args_str);

                // Parse parameters from the signature
                let parameters = parse_parameters(args_str);

                cmd_signatures.push(CMakeSignature {
                    label: full_sig,
                    documentation: content.trim().to_string(),
                    parameters,
                });
            }
        }

        // If no signatures found, create a basic one
        if cmd_signatures.is_empty() {
            cmd_signatures.push(CMakeSignature {
                label: format!("{}(...)", key),
                documentation: content.trim().to_string(),
                parameters: vec![],
            });
        }

        signatures.insert(key.clone(), cmd_signatures);
    }

    signatures
}

/// Parse parameters from a signature argument string
fn parse_parameters(args_str: &str) -> Vec<String> {
    let mut parameters = Vec::new();

    // Handle multiline signatures - normalize whitespace
    let normalized = args_str
        .lines()
        .map(|l| l.trim())
        .collect::<Vec<_>>()
        .join(" ");

    // Split by spaces but respect <angle brackets> and [square brackets]
    let mut current_param = String::new();
    let mut angle_depth = 0;
    let mut square_depth = 0;

    for ch in normalized.chars() {
        match ch {
            '<' => {
                angle_depth += 1;
                current_param.push(ch);
            }
            '>' => {
                angle_depth -= 1;
                current_param.push(ch);
            }
            '[' => {
                square_depth += 1;
                current_param.push(ch);
            }
            ']' => {
                square_depth -= 1;
                current_param.push(ch);
            }
            ' ' | '\t' if angle_depth == 0 && square_depth == 0 => {
                let trimmed = current_param.trim();
                if !trimmed.is_empty() {
                    parameters.push(trimmed.to_string());
                }
                current_param.clear();
            }
            _ => {
                current_param.push(ch);
            }
        }
    }

    // Don't forget the last parameter
    let trimmed = current_param.trim();
    if !trimmed.is_empty() {
        parameters.push(trimmed.to_string());
    }

    parameters
}

/// Lazy-loaded signature storage
pub static COMMAND_SIGNATURES: LazyLock<HashMap<String, Vec<CMakeSignature>>> = LazyLock::new(|| {
    if let Ok(output) = Command::new("cmake").arg("--help-commands").output() {
        let temp = String::from_utf8_lossy(&output.stdout);
        parse_signatures_from_help(&temp)
    } else {
        HashMap::new()
    }
});

/// Initialize signature data (called at startup)
pub fn init_signatures() {
    let _ = &*COMMAND_SIGNATURES;
}

/// Find the command name at the current position
fn find_command_at_position(source: &str, position: Position) -> Option<(String, u32)> {
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&TREESITTER_CMAKE_LANGUAGE).ok()?;
    let tree = parser.parse(source, None)?;
    let point = position.to_point();

    find_command_in_tree(tree.root_node(), point, &source.lines().collect())
}

/// Recursively find command at the given point
fn find_command_in_tree<'a>(
    node: Node<'a>,
    point: Point,
    source: &Vec<&str>,
) -> Option<(String, u32)> {
    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        // Check if point is within this node's range
        let start = child.start_position();
        let end = child.end_position();

        if point.row < start.row || point.row > end.row {
            continue;
        }
        if point.row == start.row && point.column < start.column {
            continue;
        }
        if point.row == end.row && point.column > end.column {
            continue;
        }

        // If this is a normal_command, extract the command name and argument index
        if child.kind() == CMakeNodeKinds::NORMAL_COMMAND {
            // First child is the identifier (command name)
            if let Some(id_node) = child.child(0) {
                let row = id_node.start_position().row;
                let start_col = id_node.start_position().column;
                let end_col = id_node.end_position().column;

                if row < source.len() && end_col <= source[row].len() {
                    let cmd_name = source[row][start_col..end_col].to_lowercase();

                    // Find argument index based on cursor position
                    let arg_index = find_argument_index(child, point, source);

                    return Some((cmd_name, arg_index));
                }
            }
        }

        // Recurse into children
        if let Some(result) = find_command_in_tree(child, point, source) {
            return Some(result);
        }
    }

    None
}

/// Find which argument index the cursor is at
fn find_argument_index(command_node: Node, point: Point, _source: &Vec<&str>) -> u32 {
    let mut cursor = command_node.walk();
    let mut arg_index = 0u32;

    for child in command_node.children(&mut cursor) {
        if child.kind() == CMakeNodeKinds::ARGUMENT_LIST {
            let mut arg_cursor = child.walk();
            for arg_child in child.children(&mut arg_cursor) {
                if arg_child.kind() == CMakeNodeKinds::ARGUMENT
                    || arg_child.kind() == CMakeNodeKinds::UNQUOTED_ARGUMENT
                    || arg_child.kind() == CMakeNodeKinds::QUOTED_ARGUMENT
                {
                    let arg_end = arg_child.end_position();

                    // If cursor is before or at this argument's end, we're at this index
                    if point.row < arg_end.row
                        || (point.row == arg_end.row && point.column <= arg_end.column)
                    {
                        return arg_index;
                    }
                    arg_index += 1;
                }
            }
        }
    }

    arg_index
}

/// Get signature help for a position in the document
pub fn get_signature_help(source: &str, position: Position) -> Option<SignatureHelp> {
    let (cmd_name, active_param) = find_command_at_position(source, position)?;

    let signatures = COMMAND_SIGNATURES.get(&cmd_name)?;
    if signatures.is_empty() {
        return None;
    }

    let sig_infos: Vec<SignatureInformation> = signatures
        .iter()
        .map(|sig| {
            let params: Vec<ParameterInformation> = sig
                .parameters
                .iter()
                .map(|p| ParameterInformation {
                    label: ParameterLabel::Simple(p.clone()),
                    documentation: None,
                })
                .collect();

            SignatureInformation {
                label: sig.label.clone(),
                documentation: Some(Documentation::MarkupContent(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: sig.documentation.clone(),
                })),
                parameters: if params.is_empty() {
                    None
                } else {
                    Some(params)
                },
                active_parameter: None,
            }
        })
        .collect();

    Some(SignatureHelp {
        signatures: sig_infos,
        active_signature: Some(0),
        active_parameter: Some(active_param),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_parameters() {
        let params = parse_parameters("<target> <items>...");
        assert_eq!(params, vec!["<target>", "<items>..."]);

        let params2 = parse_parameters("<variable> [<value>...]");
        assert_eq!(params2, vec!["<variable>", "[<value>...]"]);
    }

    #[test]
    fn test_signature_help() {
        let source = r#"
project(MyProject)
set(MY_VAR "value")
"#;
        let pos = Position {
            line: 2,
            character: 8,
        };
        let help = get_signature_help(source, pos);
        // Should find "set" command
        assert!(help.is_some() || COMMAND_SIGNATURES.is_empty());
    }

    #[test]
    fn test_signatures_loaded() {
        // Force initialization
        init_signatures();

        // Check that signatures are loaded
        let count = COMMAND_SIGNATURES.len();
        println!("Loaded {} command signatures", count);
        assert!(count > 0, "No signatures loaded from cmake --help-commands");

        // Check for common commands
        let common_commands = ["set", "if", "project", "message", "add_executable"];
        for cmd in common_commands {
            assert!(
                COMMAND_SIGNATURES.contains_key(cmd),
                "Missing signature for common command: {}",
                cmd
            );
        }

        // Print a sample signature
        if let Some(sigs) = COMMAND_SIGNATURES.get("set") {
            println!("set command has {} signatures:", sigs.len());
            for sig in sigs {
                println!("  label: {}", sig.label);
                println!("  params: {:?}", sig.parameters);
            }
        }
    }

    #[test]
    fn test_find_command_at_position() {
        let source = "set(MY_VAR \"value\")";
        // Position inside the parentheses
        let pos = Position {
            line: 0,
            character: 5,
        };
        let result = find_command_at_position(source, pos);
        println!("find_command_at_position result: {:?}", result);
        assert!(result.is_some(), "Should find command at position");
        let (cmd_name, arg_idx) = result.unwrap();
        assert_eq!(cmd_name, "set", "Should find 'set' command");
        println!("Command: {}, arg_index: {}", cmd_name, arg_idx);
    }
}
