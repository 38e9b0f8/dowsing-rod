use crate::types::ParseError;
use anyhow::Result;
use rustpython_parser::{ast, parse, source_code::RandomLocator, Mode};
use std::path::Path;

/// Result of parsing a single Python file.
pub struct ParseResult {
    /// The parsed AST module body (list of statements), if parsing succeeded.
    pub module: Option<Vec<ast::Stmt>>,
    /// The raw source code.
    pub source: String,
    /// Parse errors encountered (file may still partially parse in some cases).
    pub errors: Vec<ParseError>,
}

/// Parse a Python source file into an AST.
///
/// On parse error, returns the error information but does NOT panic.
/// The caller can decide whether to abort or continue scanning.
pub fn parse_python_file(path: &Path) -> Result<ParseResult> {
    let source = std::fs::read_to_string(path)?;
    parse_python_source(&source, path)
}

/// Parse Python source code string into an AST.
pub fn parse_python_source(source: &str, path: &Path) -> Result<ParseResult> {
    let file_name = path.display().to_string();

    match parse(source, Mode::Module, &file_name) {
        Ok(parsed) => {
            // Extract the module body from the parsed AST
            if let ast::Mod::Module(module) = parsed {
                Ok(ParseResult {
                    module: Some(module.body),
                    source: source.to_string(),
                    errors: Vec::new(),
                })
            } else {
                Ok(ParseResult {
                    module: None,
                    source: source.to_string(),
                    errors: vec![ParseError {
                        file: path.to_path_buf(),
                        line: None,
                        column: None,
                        message: "unexpected AST mode (expected Module)".to_string(),
                    }],
                })
            }
        }
        Err(e) => {
            let mut locator = RandomLocator::new(source);
            let location = locator.locate(e.offset);
            let error = ParseError {
                file: path.to_path_buf(),
                line: Some(location.row.to_usize()),
                column: Some(location.column.to_usize()),
                message: format!("{}", e.error),
            };
            Ok(ParseResult {
                module: None,
                source: source.to_string(),
                errors: vec![error],
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_parse_valid_python() {
        let source = "def foo(x):\n    return x + 1\n";
        let path = PathBuf::from("test.py");
        let result = parse_python_source(source, &path).unwrap();
        assert!(result.module.is_some());
        assert!(result.errors.is_empty());
    }

    #[test]
    fn test_parse_syntax_error() {
        let source = "def foo(\n";
        let path = PathBuf::from("bad.py");
        let result = parse_python_source(source, &path).unwrap();
        assert!(result.module.is_none());
        assert!(!result.errors.is_empty());
        assert!(result.errors[0].line.is_some());
    }

    #[test]
    fn test_parse_empty_file() {
        let source = "";
        let path = PathBuf::from("empty.py");
        let result = parse_python_source(source, &path).unwrap();
        assert!(result.module.is_some());
        let body = result.module.unwrap();
        assert!(body.is_empty());
    }

    #[test]
    fn test_parse_unicode() {
        let source = "def grüße(名前):\n    return f'こんにちは {名前}'\n";
        let path = PathBuf::from("unicode.py");
        let result = parse_python_source(source, &path).unwrap();
        assert!(result.module.is_some());
        assert!(result.errors.is_empty());
    }
}
