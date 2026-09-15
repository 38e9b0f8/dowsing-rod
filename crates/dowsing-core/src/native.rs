//! Tree-sitter frontends. Grammars are compiled into the binary; source stays local.
use crate::{language::Language, types::*, FileAnalysis};
use anyhow::{Context, Result};
use std::{
    borrow::Cow,
    collections::{BTreeSet, HashMap},
    path::Path,
};
use tree_sitter::{Node, Parser, Tree};

const MAX_PARSE_ERRORS_PER_FILE: usize = 20;

pub(crate) fn analyze(
    source: &str,
    path: &Path,
    language: Language,
    level: NormalizationLevel,
) -> Result<FileAnalysis> {
    let parser_input = if language == Language::Rust {
        mask_rust_primitive_macros(source)
    } else {
        Cow::Borrowed(source)
    };
    let (language, initial_tree) = select_grammar(&parser_input, path, language)?;
    if initial_tree.root_node().has_error()
        && matches!(language, Language::Verilog | Language::SystemVerilog)
    {
        // Preprocessors and UVM/DPI syntax are valid HDL input but are outside
        // the embedded grammar's useful coverage. Keep a conservative outline
        // instead of emitting false syntax errors or scoring guessed ASTs.
        return Ok(recover_verilog_outline(source, path, language));
    }
    if initial_tree.root_node().has_error() && language == Language::Vhdl {
        return Ok(recover_vhdl_outline(source, path));
    }

    let tree = initial_tree;
    // Masking preserves byte offsets; extraction always reads the original source.
    let parser_source = source;
    let parser_recovered = false;
    let mut result = FileAnalysis::default();
    let mut omitted_errors = 0usize;
    let mut first_omitted_position = None;
    let mut pending = vec![tree.root_node()];
    while let Some(node) = pending.pop() {
        if node.is_error() || node.is_missing() {
            // Macro token trees need not be standalone Rust syntax.
            if language == Language::Rust && has_ancestor_kind(node, &["macro_invocation"]) {
                continue;
            }
            if result.parse_errors.len() < MAX_PARSE_ERRORS_PER_FILE {
                result.parse_errors.push(ParseError {
                    file: path.to_path_buf(),
                    line: Some(node.start_position().row + 1),
                    column: Some(node.start_position().column + 1),
                    message: if node.is_missing() {
                        format!("missing {}", node.kind())
                    } else {
                        "unrecognized syntax".into()
                    },
                });
            } else {
                omitted_errors += 1;
                first_omitted_position.get_or_insert(node.start_position());
            }
            // One diagnostic per erroneous region, without floods from its children.
            continue;
        }
        if let Some(kind) = unit_kind(node, language) {
            // Error recovery can manufacture plausible functions. Only analyze intact units.
            if !has_reportable_error(node, language) {
                let (info, normalized) = extract(
                    node,
                    kind,
                    result.functions.len(),
                    ExtractOptions {
                        source: parser_source,
                        path,
                        language,
                        level,
                        parser_recovered,
                    },
                );
                result
                    .fingerprints
                    .push(crate::fingerprint::generate_fingerprints(
                        &normalized,
                        &info,
                    ));
                result.functions.push(info);
                result.normalized.push(normalized);
            }
        }
        let mut cursor = node.walk();
        let children: Vec<_> = node.children(&mut cursor).collect();
        pending.extend(children.into_iter().rev());
    }
    if omitted_errors > 0 {
        let position = first_omitted_position.expect("an omitted error has a position");
        result.parse_errors.push(ParseError {
            file: path.to_path_buf(),
            line: Some(position.row + 1),
            column: Some(position.column + 1),
            message: format!("{omitted_errors} additional parser diagnostics omitted"),
        });
    }
    Ok(result)
}

fn has_ancestor_kind(mut node: Node<'_>, kinds: &[&str]) -> bool {
    while let Some(parent) = node.parent() {
        if kinds.contains(&parent.kind()) {
            return true;
        }
        node = parent;
    }
    false
}

fn has_reportable_error(root: Node<'_>, language: Language) -> bool {
    let mut pending = vec![root];
    while let Some(node) = pending.pop() {
        if node.is_error() || node.is_missing() {
            if language != Language::Rust || !has_ancestor_kind(node, &["macro_invocation"]) {
                return true;
            }
            continue;
        }
        let mut cursor = node.walk();
        pending.extend(node.children(&mut cursor));
    }
    false
}

/// Tree-sitter-rust tokenizes primitive type names separately from identifiers,
/// although Rust permits macros such as `str!`. Mask the first byte so the
/// grammar recognizes the invocation while keeping every source offset stable.
fn mask_rust_primitive_macros(source: &str) -> Cow<'_, str> {
    const PRIMITIVES: &[&[u8]] = &[
        b"str", b"bool", b"char", b"i8", b"i16", b"i32", b"i64", b"i128", b"isize", b"u8", b"u16",
        b"u32", b"u64", b"u128", b"usize", b"f32", b"f64",
    ];
    let source_bytes = source.as_bytes();
    let mut masked = None;
    let mut index = 0usize;
    while index < source_bytes.len() {
        let at_boundary = index == 0
            || !source_bytes[index - 1].is_ascii_alphanumeric() && source_bytes[index - 1] != b'_';
        if at_boundary {
            if let Some(primitive) = PRIMITIVES.iter().find(|primitive| {
                source_bytes[index..].starts_with(primitive)
                    && source_bytes
                        .get(index + primitive.len())
                        .is_none_or(|byte| !byte.is_ascii_alphanumeric() && *byte != b'_')
                    && source_bytes[index + primitive.len()..]
                        .iter()
                        .find(|byte| !byte.is_ascii_whitespace())
                        == Some(&b'!')
            }) {
                masked.get_or_insert_with(|| source_bytes.to_vec())[index] = b'_';
                index += primitive.len();
                continue;
            }
        }
        index += 1;
    }
    match masked {
        Some(bytes) => Cow::Owned(String::from_utf8(bytes).expect("masking preserves UTF-8")),
        None => Cow::Borrowed(source),
    }
}

/// Recover declarations from valid HDL which the embedded grammar cannot
/// represent. These entries deliberately have no normalized form or
/// fingerprint, so they contribute to discovery statistics but never to
/// candidate generation or clustering.
fn recover_verilog_outline(source: &str, path: &Path, language: Language) -> FileAnalysis {
    let lines: Vec<_> = source
        .split_inclusive('\n')
        .scan(0usize, |offset, line| {
            let start = *offset;
            *offset += line.len();
            Some((start, line))
        })
        .collect();
    let mut result = FileAnalysis::default();
    let mut class_scopes = Vec::new();
    let mut in_macro = false;
    let mut in_block_comment = false;
    let mut line_index = 0;

    while line_index < lines.len() {
        let (start_byte, line) = lines[line_index];
        let trimmed = line.trim_start();
        if in_macro || trimmed.starts_with('`') {
            in_macro = trimmed.trim_end().ends_with('\\');
            line_index += 1;
            continue;
        }
        let stripped = strip_verilog_line(line, &mut in_block_comment);
        if stripped.is_empty() {
            line_index += 1;
            continue;
        }
        if let Some(name) = class_name(&stripped) {
            class_scopes.push(name.to_string());
        }
        if starts_word(&stripped, "endclass") {
            class_scopes.pop();
        }

        if let Some(kind) = subroutine_kind(&stripped) {
            let name = subroutine_name(&stripped).unwrap_or_else(|| {
                format!(
                    "{}@{}",
                    if kind == FunctionKind::Task {
                        "task"
                    } else {
                        "function"
                    },
                    line_index + 1
                )
            });
            let closing = if kind == FunctionKind::Task {
                "endtask"
            } else {
                "endfunction"
            };
            let Some(end_index) = find_verilog_end(&lines, line_index, closing) else {
                // DPI imports, extern declarations, and prototypes have no
                // local body to inventory.
                line_index += 1;
                continue;
            };
            result.functions.push(recovered_info(
                path,
                language,
                kind,
                name,
                &class_scopes,
                line_index + 1,
                end_index + 1,
                start_byte,
                lines[end_index].0 + lines[end_index].1.len(),
            ));
            line_index = end_index + 1;
            continue;
        }

        if let Some(kind) = procedural_kind(&stripped) {
            result.functions.push(recovered_info(
                path,
                language,
                kind,
                format!("process@{}", line_index + 1),
                &class_scopes,
                line_index + 1,
                line_index + 1,
                start_byte,
                start_byte + line.len(),
            ));
        }
        line_index += 1;
    }
    result
}

fn strip_verilog_line(line: &str, in_block_comment: &mut bool) -> String {
    let mut code = String::with_capacity(line.len());
    let mut remaining = line;
    while !remaining.is_empty() {
        if *in_block_comment {
            let Some(end) = remaining.find("*/") else {
                break;
            };
            *in_block_comment = false;
            remaining = &remaining[end + 2..];
            continue;
        }
        let block = remaining.find("/*");
        let line_comment = remaining.find("//");
        match (block, line_comment) {
            (None, Some(line_comment)) => {
                code.push_str(&remaining[..line_comment]);
                break;
            }
            (Some(block), Some(line_comment)) if line_comment < block => {
                code.push_str(&remaining[..line_comment]);
                break;
            }
            (Some(block), _) => {
                code.push_str(&remaining[..block]);
                *in_block_comment = true;
                remaining = &remaining[block + 2..];
            }
            (None, None) => {
                code.push_str(remaining);
                break;
            }
        }
    }
    code.trim_start().to_string()
}

fn starts_word(line: &str, word: &str) -> bool {
    line.strip_prefix(word)
        .is_some_and(|rest| rest.is_empty() || !is_identifier_byte(rest.as_bytes()[0]))
}

fn is_identifier_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'$')
}

fn keyword_at(line: &str, word: &str) -> Option<usize> {
    line.match_indices(word).find_map(|(index, _)| {
        let bytes = line.as_bytes();
        let before = index.checked_sub(1).and_then(|i| bytes.get(i));
        let after = bytes.get(index + word.len());
        (!before.is_some_and(|byte| is_identifier_byte(*byte))
            && !after.is_some_and(|byte| is_identifier_byte(*byte)))
        .then_some(index)
    })
}

fn subroutine_kind(line: &str) -> Option<FunctionKind> {
    if keyword_at(line, "function").is_some() {
        Some(FunctionKind::Function)
    } else if keyword_at(line, "task").is_some() {
        Some(FunctionKind::Task)
    } else {
        None
    }
}

fn subroutine_name(line: &str) -> Option<String> {
    let keyword = ["function", "task"]
        .into_iter()
        .filter_map(|word| keyword_at(line, word).map(|index| (index, word.len())))
        .min_by_key(|(index, _)| *index)?;
    let declaration = &line[keyword.0 + keyword.1..];
    let before_arguments = declaration
        .split_once('(')
        .map_or(declaration, |(before, _)| before);
    let head = before_arguments
        .split_once(';')
        .map_or(before_arguments, |(before, _)| before);
    let name = head
        .split(|character: char| {
            !(character.is_ascii_alphanumeric() || character == '_' || character == '$')
        })
        .rfind(|part| !part.is_empty())?;
    Some(name.to_string())
}

fn class_name(line: &str) -> Option<&str> {
    let index = keyword_at(line, "class")?;
    line[index + "class".len()..]
        .split(|character: char| {
            !(character.is_ascii_alphanumeric() || character == '_' || character == '$')
        })
        .find(|part| !part.is_empty())
}

fn procedural_kind(line: &str) -> Option<FunctionKind> {
    ["always", "always_ff", "always_comb", "always_latch"]
        .into_iter()
        .any(|word| starts_word(line, word))
        .then_some(FunctionKind::Process)
}

fn find_verilog_end(lines: &[(usize, &str)], start: usize, ending: &str) -> Option<usize> {
    let mut in_macro = false;
    let mut in_block_comment = false;
    for (index, (_, line)) in lines.iter().enumerate().skip(start + 1) {
        let trimmed = line.trim_start();
        if in_macro || trimmed.starts_with('`') {
            in_macro = trimmed.trim_end().ends_with('\\');
            continue;
        }
        let stripped = strip_verilog_line(line, &mut in_block_comment);
        if starts_word(&stripped, ending) {
            return Some(index);
        }
        if subroutine_kind(&stripped).is_some() {
            return None;
        }
    }
    None
}

#[allow(clippy::too_many_arguments)]
fn recovered_info(
    path: &Path,
    language: Language,
    kind: FunctionKind,
    name: String,
    scopes: &[String],
    start_line: usize,
    end_line: usize,
    start_byte: usize,
    end_byte: usize,
) -> FunctionInfo {
    let mut qualified = scopes.to_vec();
    qualified.push(name.clone());
    FunctionInfo {
        language,
        file: path.to_path_buf(),
        module: path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned(),
        qualified_name: qualified.join("::"),
        class_name: scopes.last().cloned(),
        function_name: name.clone(),
        kind,
        start_line,
        end_line,
        source_bytes: end_byte.saturating_sub(start_byte),
        ast_node_count: 0,
        complexity: 0,
        decorators: Vec::new(),
        parameters: Vec::new(),
        return_annotation: None,
        called_functions: BTreeSet::new(),
        is_public: !name.starts_with('_'),
        is_dunder: false,
        is_test: false,
        is_property: false,
        is_classmethod: false,
        is_staticmethod: false,
        is_async: false,
        parser_recovered: true,
    }
}

fn recover_vhdl_outline(source: &str, path: &Path) -> FileAnalysis {
    let lines: Vec<_> = source
        .split_inclusive('\n')
        .scan(0usize, |offset, line| {
            let start = *offset;
            *offset += line.len();
            Some((start, line))
        })
        .collect();
    let mut result = FileAnalysis::default();
    let mut line_index = 0;
    while line_index < lines.len() {
        let (start_byte, line) = lines[line_index];
        let stripped = strip_vhdl_line(line);
        if let Some((kind, keyword)) = vhdl_unit_kind(stripped) {
            let name = vhdl_unit_name(stripped, keyword)
                .unwrap_or_else(|| format!("{}@{}", keyword, line_index + 1));
            let ending = match kind {
                FunctionKind::Procedure => "procedure",
                FunctionKind::Process => "process",
                _ => "function",
            };
            let end_index = find_vhdl_end(&lines, line_index, ending).unwrap_or(line_index);
            result.functions.push(recovered_info(
                path,
                Language::Vhdl,
                kind,
                name,
                &[],
                line_index + 1,
                end_index + 1,
                start_byte,
                lines[end_index].0 + lines[end_index].1.len(),
            ));
            line_index = end_index + 1;
            continue;
        }
        line_index += 1;
    }
    result
}

fn strip_vhdl_line(line: &str) -> &str {
    line.split_once("--")
        .map_or(line, |(content, _)| content)
        .trim_start()
}

fn vhdl_unit_kind(line: &str) -> Option<(FunctionKind, &'static str)> {
    if keyword_at(line, "function").is_some() {
        Some((FunctionKind::Function, "function"))
    } else if keyword_at(line, "procedure").is_some() {
        Some((FunctionKind::Procedure, "procedure"))
    } else if keyword_at(line, "process").is_some() {
        Some((FunctionKind::Process, "process"))
    } else {
        None
    }
}

fn vhdl_unit_name(line: &str, keyword: &str) -> Option<String> {
    let index = keyword_at(line, keyword)?;
    line[index + keyword.len()..]
        .split(|character: char| !(character.is_ascii_alphanumeric() || character == '_'))
        .find(|part| !part.is_empty())
        .map(str::to_string)
}

fn find_vhdl_end(lines: &[(usize, &str)], start: usize, kind: &str) -> Option<usize> {
    lines
        .iter()
        .enumerate()
        .skip(start + 1)
        .find_map(|(index, (_, line))| {
            let line = strip_vhdl_line(line).to_ascii_lowercase();
            (starts_word(&line, "end") && keyword_at(&line, kind).is_some()).then_some(index)
        })
}

/// `.h` is inherently ambiguous. Parse it as both C and C++ and keep the
/// grammar with fewer error regions; ties preserve the C default.
fn select_grammar(source: &str, path: &Path, language: Language) -> Result<(Language, Tree)> {
    let c_tree = parse_tree(source, language.grammar(path))?;
    if language != Language::C || !path.extension().is_some_and(|ext| ext == "h") {
        return Ok((language, c_tree));
    }

    let cpp_tree = parse_tree(source, Language::Cpp.grammar(path))?;
    if error_node_count(cpp_tree.root_node()) < error_node_count(c_tree.root_node()) {
        Ok((Language::Cpp, cpp_tree))
    } else {
        Ok((language, c_tree))
    }
}

fn parse_tree(source: &str, language: tree_sitter::Language) -> Result<Tree> {
    let mut parser = Parser::new();
    parser
        .set_language(&language)
        .context("loading native grammar")?;
    parser
        .parse(source, None)
        .context("parser did not produce a syntax tree")
}

fn error_node_count(root: Node<'_>) -> usize {
    let mut errors = 0;
    let mut pending = vec![root];
    while let Some(node) = pending.pop() {
        if node.is_error() || node.is_missing() {
            errors += 1;
            continue;
        }
        let mut cursor = node.walk();
        pending.extend(node.children(&mut cursor));
    }
    errors
}

fn unit_kind(node: Node<'_>, language: Language) -> Option<FunctionKind> {
    use FunctionKind::*;
    Some(match (language, node.kind()) {
        (Language::Verilog | Language::SystemVerilog, "always_construct") => Process,
        (Language::Verilog | Language::SystemVerilog, "function_declaration") => Function,
        (Language::Verilog | Language::SystemVerilog, "task_declaration") => Task,
        (Language::SystemVerilog, "class_constructor_declaration") => Function,
        (Language::Vhdl, "process_statement") => Process,
        (Language::Vhdl, "subprogram_definition") => {
            if child_kind(node, "procedure_specification").is_some() {
                Procedure
            } else {
                Function
            }
        }
        (
            Language::TypeScript | Language::JavaScript,
            "function_declaration"
            | "function_expression"
            | "generator_function"
            | "generator_function_declaration",
        ) => Function,
        (Language::TypeScript | Language::JavaScript, "method_definition")
            if node.child_by_field_name("body").is_some() =>
        {
            Method
        }
        (Language::TypeScript | Language::JavaScript, "arrow_function") => Lambda,
        (Language::Rust, "function_item") if node.child_by_field_name("body").is_some() => Function,
        (Language::Rust, "closure_expression") => Lambda,
        (Language::C | Language::Cpp, "function_definition")
            if node.child_by_field_name("body").is_some() =>
        {
            Function
        }
        (Language::Cpp | Language::CSharp, "lambda_expression") => Lambda,
        (
            Language::CSharp,
            "method_declaration"
            | "constructor_declaration"
            | "destructor_declaration"
            | "operator_declaration"
            | "conversion_operator_declaration",
        ) if node.child_by_field_name("body").is_some() => Method,
        (Language::CSharp, "local_function_statement")
            if node.child_by_field_name("body").is_some() =>
        {
            NestedFunction
        }
        _ => return None,
    })
}

fn text<'a>(node: Node<'_>, source: &'a str) -> &'a str {
    &source[node.byte_range()]
}

fn child_kind<'a>(node: Node<'a>, kind: &str) -> Option<Node<'a>> {
    let mut cursor = node.walk();
    let found = node.named_children(&mut cursor).find(|n| n.kind() == kind);
    found
}

fn find_kind<'a>(node: Node<'a>, kinds: &[&str]) -> Option<Node<'a>> {
    let mut pending = vec![node];
    while let Some(n) = pending.pop() {
        if kinds.contains(&n.kind()) {
            return Some(n);
        }
        let mut cursor = n.walk();
        let children: Vec<_> = n.named_children(&mut cursor).collect();
        pending.extend(children.into_iter().rev());
    }
    None
}

/// Follow declarator fields instead of accidentally choosing a parameter/type name.
fn declarator_name(mut node: Node<'_>) -> Option<Node<'_>> {
    loop {
        if let Some(next) = node.child_by_field_name("declarator") {
            node = next;
        } else if matches!(
            node.kind(),
            "identifier"
                | "field_identifier"
                | "qualified_identifier"
                | "destructor_name"
                | "operator_name"
                | "structured_binding_declarator"
        ) {
            return Some(node);
        } else {
            return None;
        }
    }
}

fn unit_name(node: Node<'_>, language: Language) -> Option<Node<'_>> {
    if let Some(name) = node.child_by_field_name("name") {
        return Some(name);
    }
    match language {
        Language::C | Language::Cpp => node
            .child_by_field_name("declarator")
            .and_then(declarator_name),
        Language::Verilog | Language::SystemVerilog => {
            find_kind(node, &["function_identifier", "task_identifier"])
        }
        Language::Vhdl => {
            if let Some(spec) = child_kind(node, "function_specification")
                .or_else(|| child_kind(node, "procedure_specification"))
            {
                return spec
                    .child_by_field_name("function")
                    .or_else(|| spec.child_by_field_name("procedure"));
            }
            child_kind(node, "label_declaration")
        }
        _ => None,
    }
}

fn spelling(raw: &str, language: Language) -> String {
    // VHDL basic identifiers/keywords are case insensitive; extended identifiers are not.
    if language == Language::Vhdl && !raw.starts_with('\\') {
        raw.to_ascii_lowercase()
    } else {
        raw.to_string()
    }
}

fn parameter_nodes<'a>(node: Node<'a>, source: &str, language: Language) -> Vec<Node<'a>> {
    let container = node
        .child_by_field_name("parameters")
        .or_else(|| node.child_by_field_name("parameter"))
        .or_else(|| {
            node.child_by_field_name("declarator")
                .and_then(|d| find_kind(d, &["parameter_list"]))
        })
        .or_else(|| {
            if language.is_hdl() {
                find_kind(node, &["tf_port_list", "parameter_list_specification"])
            } else {
                None
            }
        });
    let mut parameters = Vec::new();
    if let Some(container) = container {
        if matches!(container.kind(), "identifier" | "implicit_parameter") {
            return vec![container];
        }
        let mut pending = vec![container];
        while let Some(n) = pending.pop() {
            if n.kind() == "self_parameter" {
                continue;
            }
            if matches!(
                n.kind(),
                "required_parameter"
                    | "optional_parameter"
                    | "parameter"
                    | "parameter_declaration"
                    | "optional_parameter_declaration"
            ) {
                if let Some(name) = n
                    .child_by_field_name("name")
                    .or_else(|| n.child_by_field_name("pattern"))
                    .or_else(|| {
                        n.child_by_field_name("declarator")
                            .and_then(declarator_name)
                    })
                {
                    if language == Language::Rust && name.kind() != "identifier" {
                        parameters.extend(
                            rust_pattern_bindings(name)
                                .into_iter()
                                .filter(|binding| !is_rust_non_local(text(*binding, source))),
                        );
                    } else if language != Language::Rust || !is_rust_non_local(text(name, source)) {
                        parameters.push(name);
                    }
                }
                continue;
            }
            if n.kind() == "identifier_list" {
                let mut cursor = n.walk();
                parameters.extend(n.named_children(&mut cursor));
                continue;
            }
            if matches!(n.kind(), "port_identifier" | "tf_variable_identifier") {
                parameters.push(n);
                continue;
            }
            if language == Language::Rust && is_rust_pattern_binding(n) {
                if !is_rust_non_local(text(n, source)) {
                    parameters.push(n);
                }
                continue;
            }
            let mut cursor = n.walk();
            let children: Vec<_> = n.named_children(&mut cursor).collect();
            pending.extend(children.into_iter().rev());
        }
    }
    parameters
}

fn is_rust_non_local(name: &str) -> bool {
    matches!(name, "self" | "Self" | "super" | "crate")
}

fn is_rust_pattern_binding(node: Node<'_>) -> bool {
    match node.kind() {
        "shorthand_field_identifier" => true,
        "identifier" => !node.parent().is_some_and(|parent| {
            matches!(parent.kind(), "scoped_identifier" | "generic_pattern")
                || (parent.kind() == "tuple_struct_pattern"
                    && parent.child_by_field_name("type") == Some(node))
        }),
        _ => false,
    }
}

fn rust_pattern_bindings(node: Node<'_>) -> Vec<Node<'_>> {
    let mut bindings = Vec::new();
    let mut pending = vec![node];
    while let Some(current) = pending.pop() {
        if current.kind() == "match_pattern" {
            let condition = current.child_by_field_name("condition");
            let mut cursor = current.walk();
            let children: Vec<_> = current.named_children(&mut cursor).collect();
            pending.extend(
                children
                    .into_iter()
                    .rev()
                    .filter(|child| condition != Some(*child)),
            );
            continue;
        }
        if is_rust_pattern_binding(current) {
            bindings.push(current);
            continue;
        }
        if matches!(
            current.kind(),
            "type_identifier" | "primitive_type" | "lifetime" | "macro_invocation"
        ) {
            continue;
        }
        let type_field = current.child_by_field_name("type");
        let mut cursor = current.walk();
        let children: Vec<_> = current.named_children(&mut cursor).collect();
        pending.extend(
            children
                .into_iter()
                .rev()
                .filter(|child| type_field != Some(*child)),
        );
    }
    bindings
}

struct ExtractOptions<'a> {
    source: &'a str,
    path: &'a Path,
    language: Language,
    level: NormalizationLevel,
    parser_recovered: bool,
}

fn extract(
    node: Node<'_>,
    mut kind: FunctionKind,
    id: usize,
    options: ExtractOptions<'_>,
) -> (FunctionInfo, NormalizedFunction) {
    let ExtractOptions {
        source,
        path,
        language,
        level,
        parser_recovered,
    } = options;
    let name_node = if kind == FunctionKind::Process && language != Language::Vhdl {
        None
    } else {
        unit_name(node, language)
    };
    let name = name_node
        .map(|n| text(n, source).trim_end_matches(':').trim().to_string())
        .or_else(|| {
            (language == Language::SystemVerilog && node.kind() == "class_constructor_declaration")
                .then_some("new".to_string())
        })
        .or_else(|| {
            node.parent()
                .filter(|p| matches!(p.kind(), "variable_declarator" | "init_declarator" | "pair"))
                .and_then(|p| {
                    p.child_by_field_name("name")
                        .or_else(|| p.child_by_field_name("declarator"))
                        .or_else(|| p.child_by_field_name("key"))
                })
                .map(|n| text(n, source).to_string())
        })
        .unwrap_or_else(|| {
            format!(
                "{}@{}:{}",
                if kind == FunctionKind::Process {
                    "process"
                } else {
                    "lambda"
                },
                node.start_position().row + 1,
                node.start_position().column + 1
            )
        });
    if matches!(language, Language::Cpp | Language::Rust) && kind == FunctionKind::Function {
        let mut parent = node.parent();
        while let Some(p) = parent {
            if language == Language::Rust && unit_kind(p, language).is_some() {
                break;
            }
            if matches!(
                p.kind(),
                "class_specifier" | "struct_specifier" | "impl_item" | "trait_item"
            ) {
                kind = FunctionKind::Method;
                break;
            }
            parent = p.parent();
        }
    }
    let mut scopes = Vec::new();
    let mut class_name = None;
    let mut parent = node.parent();
    while let Some(p) = parent {
        if matches!(
            p.kind(),
            "class_declaration"
                | "class_specifier"
                | "struct_specifier"
                | "struct_declaration"
                | "record_declaration"
                | "namespace_definition"
                | "namespace_declaration"
                | "module_declaration"
                | "architecture_definition"
                | "package_definition"
                | "impl_item"
                | "mod_item"
                | "trait_item"
        ) || unit_kind(p, language).is_some()
        {
            let scope = p
                .child_by_field_name("name")
                .or_else(|| p.child_by_field_name("architecture"))
                .or_else(|| {
                    (p.kind() == "impl_item")
                        .then(|| p.child_by_field_name("type"))
                        .flatten()
                })
                .or_else(|| {
                    matches!(
                        p.kind(),
                        "class_declaration" | "class_specifier" | "struct_specifier"
                    )
                    .then(|| find_kind(p, &["class_identifier", "type_identifier"]))
                    .flatten()
                })
                .or_else(|| {
                    child_kind(p, "module_header")
                        .or_else(|| child_kind(p, "module_nonansi_header"))
                        .or_else(|| child_kind(p, "module_ansi_header"))
                        .and_then(|n| find_kind(n, &["simple_identifier", "escaped_identifier"]))
                })
                .or_else(|| unit_name(p, language));
            if let Some(scope) = scope {
                let scope = text(scope, source).to_string();
                if class_name.is_none()
                    && (p.kind().contains("class")
                        || matches!(p.kind(), "impl_item" | "trait_item"))
                {
                    class_name = Some(scope.clone());
                }
                scopes.push(scope);
            }
            if unit_kind(p, language).is_some() && kind == FunctionKind::Function {
                kind = FunctionKind::NestedFunction;
            }
        }
        parent = p.parent();
    }
    scopes.reverse();
    scopes.push(name.clone());
    let parameters: Vec<String> = parameter_nodes(node, source, language)
        .into_iter()
        .map(|n| spelling(text(n, source), language))
        .collect();
    let mut normalizer = Normalizer {
        source,
        language,
        level,
        root: node,
        skip: name_node,
        locals: vec![HashMap::new()],
        next_local: 0,
        tokens: Vec::new(),
        calls: BTreeSet::new(),
        complexity: 1,
        nodes: 0,
    };
    for p in &parameters {
        normalizer.bind(p.clone());
    }
    // Verilog functions can return by assigning to their own name.
    if matches!(language, Language::Verilog | Language::SystemVerilog)
        && kind == FunctionKind::Function
    {
        normalizer.bind(spelling(&name, language));
    }
    normalizer.normalize();
    let is_async = normalizer
        .tokens
        .iter()
        .any(|t| matches!(t, StructuralToken::Syntax(s) if s == "async"));
    if is_async {
        kind = if kind == FunctionKind::Method {
            FunctionKind::AsyncMethod
        } else {
            FunctionKind::AsyncFunction
        };
    }
    let info = FunctionInfo {
        language,
        file: path.to_path_buf(),
        module: path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned(),
        qualified_name: scopes.join("::"),
        class_name,
        function_name: name.clone(),
        kind,
        start_line: node.start_position().row + 1,
        end_line: node.end_position().row + 1,
        source_bytes: node.end_byte() - node.start_byte(),
        ast_node_count: normalizer.nodes,
        complexity: normalizer.complexity,
        decorators: Vec::new(),
        parameters,
        return_annotation: node
            .child_by_field_name("return_type")
            .or_else(|| node.child_by_field_name("returns"))
            .or_else(|| node.child_by_field_name("type"))
            .map(|n| text(n, source).to_string()),
        called_functions: normalizer.calls,
        is_public: !name.starts_with('_'),
        is_dunder: false,
        is_test: name.starts_with("test")
            || path
                .components()
                .any(|p| matches!(p.as_os_str().to_str(), Some("tests" | "test"))),
        is_property: false,
        is_classmethod: false,
        is_staticmethod: false,
        is_async,
        parser_recovered,
    };
    (
        info,
        NormalizedFunction {
            function_id: id,
            tokens: normalizer.tokens,
        },
    )
}

struct Normalizer<'a> {
    source: &'a str,
    language: Language,
    level: NormalizationLevel,
    root: Node<'a>,
    skip: Option<Node<'a>>,
    locals: Vec<HashMap<String, usize>>,
    next_local: usize,
    tokens: Vec<StructuralToken>,
    calls: BTreeSet<String>,
    complexity: usize,
    nodes: usize,
}

impl Normalizer<'_> {
    fn bind(&mut self, name: String) {
        let scope = self.locals.last_mut().unwrap();
        if let std::collections::hash_map::Entry::Vacant(entry) = scope.entry(name) {
            entry.insert(self.next_local);
            self.next_local += 1;
        }
    }

    fn register(&mut self, node: Node<'_>) {
        if self.language == Language::Rust {
            if let Some(pattern) = match node.kind() {
                "let_declaration" | "for_expression" | "match_arm" => {
                    node.child_by_field_name("pattern")
                }
                _ => None,
            } {
                for binding in rust_pattern_bindings(pattern) {
                    let name = spelling(text(binding, self.source), self.language);
                    if !is_rust_non_local(&name) {
                        self.bind(name);
                    }
                }
            }
        }
        let binding = match node.kind() {
            "variable_declarator" => node.child_by_field_name("name"),
            "for_in_statement" => node.child_by_field_name("left"),
            "init_declarator" => node
                .child_by_field_name("declarator")
                .and_then(declarator_name),
            "declaration" if matches!(self.language, Language::C | Language::Cpp) => node
                .child_by_field_name("declarator")
                .and_then(declarator_name),
            "variable_declaration" | "constant_declaration" if self.language == Language::Vhdl => {
                child_kind(node, "identifier_list")
            }
            "variable_decl_assignment" => find_kind(node, &["simple_identifier"]),
            _ => None,
        };
        if let Some(n) = binding {
            if n.kind() == "identifier_list" {
                let mut cursor = n.walk();
                for child in n.named_children(&mut cursor) {
                    self.bind(spelling(text(child, self.source), self.language));
                }
            } else {
                self.bind(spelling(text(n, self.source), self.language));
            }
        }
    }

    fn normalize(&mut self) {
        // An explicit stack avoids overflowing on generated or deeply nested source.
        let mut pending = vec![(self.root, false, false)];
        while let Some((node, exit, scope)) = pending.pop() {
            if exit {
                self.tokens.push(StructuralToken::BlockEnd);
                if scope {
                    self.locals.pop();
                }
                continue;
            }
            if Some(node) == self.skip
                || node.kind().contains("comment")
                || matches!(
                    node.kind(),
                    "label_declaration" | "end_process" | "subprogram_end"
                )
            {
                continue;
            }
            if node != self.root && unit_kind(node, self.language).is_some() {
                self.tokens.push(StructuralToken::FunctionDef);
                continue;
            }
            self.nodes += usize::from(node.is_named());
            let is_scope = node != self.root
                && matches!(
                    node.kind(),
                    "statement_block"
                        | "compound_statement"
                        | "block"
                        | "seq_block"
                        | "for_statement"
                        | "for_in_statement"
                        | "for_expression"
                        | "loop_expression"
                        | "while_expression"
                        | "match_expression"
                        | "match_arm"
                );
            if is_scope {
                self.locals.push(HashMap::new());
            }
            self.register(node);
            let raw = text(node, self.source);
            let value = spelling(raw, self.language);
            if let Some(call) = call_target(node, self.source, self.language) {
                self.calls.insert(call.clone());
                self.tokens.push(StructuralToken::Call(call));
            }
            if let Some(token) = control_token(node.kind(), &value) {
                if !matches!(
                    token,
                    StructuralToken::Return
                        | StructuralToken::Break
                        | StructuralToken::Continue
                        | StructuralToken::Else
                ) {
                    self.complexity += 1;
                }
                self.tokens.push(token);
            }
            if literal_kind(node.kind()) {
                self.tokens
                    .push(if self.level == NormalizationLevel::Aggressive {
                        if node.kind().contains("string") || node.kind().contains("char") {
                            StructuralToken::StringLiteral
                        } else {
                            StructuralToken::NumericLiteral
                        }
                    } else {
                        StructuralToken::Literal(raw.to_string())
                    });
                if is_scope {
                    self.locals.pop();
                }
                continue;
            }
            if node.child_count() == 0 {
                let binding = (self.level != NormalizationLevel::Strict
                    && is_identifier(node.kind())
                    && !is_member(node))
                .then(|| {
                    self.locals
                        .iter()
                        .rev()
                        .find_map(|scope| scope.get(&value).copied())
                })
                .flatten();

                if let Some(binding_id) = binding {
                    self.tokens.push(StructuralToken::LocalBinding(binding_id));
                } else if is_identifier(node.kind()) {
                    self.tokens.push(StructuralToken::ExternalName(value));
                } else {
                    self.tokens.push(StructuralToken::Syntax(value));
                }
                if is_scope {
                    self.locals.pop();
                }
                continue;
            }
            self.tokens
                .push(StructuralToken::Syntax(node.kind().to_string()));
            self.tokens.push(StructuralToken::BlockStart);
            pending.push((node, true, is_scope));
            let mut cursor = node.walk();
            let children: Vec<_> = node.children(&mut cursor).collect();
            pending.extend(children.into_iter().rev().map(|n| (n, false, false)));
        }
    }
}

fn is_identifier(kind: &str) -> bool {
    matches!(
        kind,
        "identifier"
            | "simple_identifier"
            | "escaped_identifier"
            | "property_identifier"
            | "field_identifier"
            | "shorthand_property_identifier"
            | "implicit_parameter"
            | "library_constant"
            | "library_function"
            | "library_type"
    )
}

fn is_member(node: Node<'_>) -> bool {
    if matches!(node.kind(), "property_identifier" | "field_identifier") {
        return true;
    }
    node.parent().is_some_and(|p| {
        matches!(
            p.kind(),
            "member_access_expression"
                | "member_expression"
                | "field_expression"
                | "qualified_identifier"
                | "selection"
                | "scoped_identifier"
                | "scoped_type_identifier"
        ) && p
            .child_by_field_name("expression")
            .or_else(|| p.child_by_field_name("object"))
            .or_else(|| p.child_by_field_name("argument"))
            .or_else(|| p.child_by_field_name("value"))
            .or_else(|| p.child_by_field_name("path"))
            != Some(node)
    })
}

fn literal_kind(kind: &str) -> bool {
    matches!(
        kind,
        "number"
            | "number_literal"
            | "integer_literal"
            | "real_literal"
            | "decimal_integer"
            | "decimal_integer_literal"
            | "hex_integer_literal"
            | "decimal_real_literal"
            | "string"
            | "string_literal"
            | "raw_string_literal"
            | "character_literal"
            | "char_literal"
            | "float_literal"
            | "boolean_literal"
            | "bit_string_literal"
            | "based_literal"
            | "decimal_literal"
            | "integral_number"
            | "real_number"
    )
}

fn control_token(kind: &str, value: &str) -> Option<StructuralToken> {
    use StructuralToken::*;
    Some(match kind {
        "if_statement"
        | "conditional_statement"
        | "conditional_expression"
        | "if_else_statement"
        | "if_expression" => If,
        "for_statement" | "for_in_statement" | "for_range_loop" | "foreach_statement"
        | "loop_statement" | "for_expression" => For,
        "while_statement" | "do_statement" | "loop_expression" | "while_expression" => While,
        "switch_statement" | "case_statement" | "match_expression" => Match,
        "case_item" | "case_statement_alternative" | "switch_section" | "match_arm" => Case,
        "return_statement" | "return_expression" => Return,
        "break_statement" | "break_expression" => Break,
        "continue_statement" | "continue_expression" => Continue,
        "try_statement" => Try,
        "try_expression" | "throw_statement" => Raise,
        "await_expression" => Await,
        "catch_clause" => ExceptHandler,
        "else" if value == "else" => Else,
        _ => return None,
    })
}

fn call_target(node: Node<'_>, source: &str, language: Language) -> Option<String> {
    let target = match node.kind() {
        "call_expression" => node.child_by_field_name("function"),
        "macro_invocation" => node.child_by_field_name("macro"),
        "invocation_expression" => node.child_by_field_name("function"),
        "tf_call" | "system_tf_call" => node.named_child(0),
        "name"
            if language == Language::Vhdl
                && (child_kind(node, "function_call").is_some()
                    || (child_kind(node, "parenthesis_group").is_some()
                        && node
                            .named_child(0)
                            .is_some_and(|n| n.kind() == "library_function"))) =>
        {
            node.named_child(0)
        }
        "procedure_call_statement" if language == Language::Vhdl => {
            child_kind(node, "name").and_then(|n| n.named_child(0))
        }
        _ => None,
    }?;
    Some(spelling(text(target, source), language))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn run(source: &str, extension: &str, level: NormalizationLevel) -> FileAnalysis {
        let path = std::path::PathBuf::from(format!("fixture.{extension}"));
        analyze(source, &path, Language::from_path(&path).unwrap(), level).unwrap()
    }
    #[test]
    fn renamed_bindings_normalize_in_every_language() {
        for file in [
            "duplicates.ts",
            "duplicates.c",
            "duplicates.cpp",
            "duplicates.cs",
            "duplicates.rs",
            "duplicates.vhd",
        ] {
            let path = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../tests/languages")
                .join(file);
            let source = std::fs::read_to_string(&path).unwrap();
            let result = analyze(
                &source,
                &path,
                Language::from_path(&path).unwrap(),
                NormalizationLevel::Balanced,
            )
            .unwrap();
            assert!(result.parse_errors.is_empty(), "{file}");
            assert_eq!(
                result.normalized[0].tokens, result.normalized[1].tokens,
                "{file}"
            );
            let strict = analyze(
                &source,
                &path,
                Language::from_path(&path).unwrap(),
                NormalizationLevel::Strict,
            )
            .unwrap();
            assert_ne!(
                strict.normalized[0].tokens, strict.normalized[1].tokens,
                "strict {file}"
            );
        }
    }
    #[test]
    fn bindings_preserve_data_flow_and_shadowing() {
        let r = run(
            "int a(int x, int y) { return x - y; } int b(int x, int y) { return y - x; }",
            "c",
            NormalizationLevel::Balanced,
        );
        assert_ne!(r.normalized[0].tokens, r.normalized[1].tokens);
        let r = run("int a(int x) { { int x = 2; use(x); } return x; } int b(int y) { { int z = 2; use(z); } return y; }", "c", NormalizationLevel::Balanced);
        assert_eq!(r.normalized[0].tokens, r.normalized[1].tokens);
    }
    #[test]
    fn literals_operators_members_and_calls_survive() {
        for other in [
            "return input + 2;",
            "return input - 1;",
            "return save(input);",
            "return input.deleted;",
        ] {
            let source = format!("function a(input: any) {{ return input + 1; }} function b(input: any) {{ {other} }}");
            let r = run(&source, "ts", NormalizationLevel::Balanced);
            assert_ne!(r.normalized[0].tokens, r.normalized[1].tokens);
        }
        let r = run(
            "function a(x: number) { return x + 1; } function b(y: number) { return y + 2; }",
            "ts",
            NormalizationLevel::Aggressive,
        );
        assert_eq!(r.normalized[0].tokens, r.normalized[1].tokens);
    }
    #[test]
    fn hdl_timing_and_assignment_semantics_survive() {
        for variant in [
            "always_ff @(negedge clk) q <= data;",
            "always_comb q = data;",
            "always_ff @(posedge clk) q = data;",
        ] {
            let source = format!("module m(input clk, data, output reg q); always_ff @(posedge clk) q <= data; {variant} endmodule");
            let r = run(&source, "sv", NormalizationLevel::Balanced);
            assert!(r.parse_errors.is_empty(), "{variant}");
            assert_eq!(r.functions.len(), 2);
            assert_ne!(r.normalized[0].tokens, r.normalized[1].tokens);
        }
    }
    #[test]
    fn errors_skip_broken_units_but_keep_valid_siblings() {
        for (ext, source) in [
            (
                "ts",
                "function good(x: number) { return x; } function broken( {",
            ),
            ("c", "int good(int x) { return x; } int broken( {"),
            ("cpp", "int good(int x) { return x; } int broken( {"),
            (
                "cs",
                "class A { int Good(int x) { return x; } int Broken( { }",
            ),
            ("rs", "fn good(x: i32) -> i32 { x } fn broken( {"),
        ] {
            let r = run(source, ext, NormalizationLevel::Balanced);
            assert!(!r.parse_errors.is_empty(), "{ext}");
            assert!(r
                .parse_errors
                .iter()
                .all(|e| e.line.is_some() && e.column.is_some()));
        }
        let r = run(
            "function good(x: number) { return x; } function broken(x: number) { return x + ; }",
            "ts",
            NormalizationLevel::Balanced,
        );
        assert_eq!(r.functions.len(), 1);
        assert_eq!(r.functions[0].function_name, "good");
    }
    #[test]
    fn hdl_fallback_is_discovery_only() {
        for (extension, source) in [
            (
                "svh",
                "class item;\nvirtual task drive();\n`uvm_info(\"I\", \"go\", UVM_LOW)\nendtask\nendclass",
            ),
            (
                "vhd",
                "architecture rtl of fifo is begin checks : block begin assert always ready; end block; process(clk) begin end process; end architecture;",
            ),
        ] {
            let result = run(source, extension, NormalizationLevel::Balanced);
            assert!(result.parse_errors.is_empty(), "{extension}");
            assert!(!result.functions.is_empty(), "{extension}");
            assert!(result.normalized.is_empty(), "{extension}");
            assert!(result
                .functions
                .iter()
                .all(|function| function.parser_recovered));
        }
    }

    #[test]
    fn recovered_systemverilog_skips_macros_and_dpi_declarations() {
        let source = r#"`define DECLARE_HELPER \
function void generated(); endfunction
import "DPI-C" function void foreign_call();
function void local_helper();
endfunction
"#;
        let result = run(source, "sv", NormalizationLevel::Balanced);
        assert!(result.parse_errors.is_empty());
        assert_eq!(result.functions.len(), 1);
        assert_eq!(result.functions[0].function_name, "local_helper");
        assert!(result.functions[0].parser_recovered);
    }
    #[test]
    fn nested_bodies_do_not_pollute_parent_calls() {
        let r = run("function outer(x: number) { function inner(y: number) { return danger(y); } return safe(x); }", "ts", NormalizationLevel::Balanced);
        assert_eq!(r.functions.len(), 2);
        assert_eq!(r.functions[1].qualified_name, "outer::inner");
        assert!(r.functions[0].called_functions.contains("safe"));
        assert!(!r.functions[0].called_functions.contains("danger"));
    }
    #[test]
    fn rust_impl_methods_and_trait_signatures() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/languages")
            .join("duplicates.rs");
        let source = std::fs::read_to_string(&path).unwrap();
        let result = analyze(&source, &path, Language::Rust, NormalizationLevel::Balanced).unwrap();
        assert!(result.parse_errors.is_empty());
        assert_eq!(result.functions.len(), 7);
        let first = result
            .functions
            .iter()
            .find(|function| function.function_name == "first")
            .unwrap();
        let second = result
            .functions
            .iter()
            .find(|function| function.function_name == "second")
            .unwrap();
        assert_eq!(first.kind, FunctionKind::Method);
        assert_eq!(second.kind, FunctionKind::Method);
        assert_eq!(first.class_name.as_deref(), Some("Calculator"));
        assert_eq!(second.class_name.as_deref(), Some("Calculator"));
        assert!(result
            .functions
            .iter()
            .all(|function| function.function_name != "required"));
        let fetch_one = result
            .functions
            .iter()
            .find(|function| function.function_name == "fetch_one")
            .unwrap();
        assert_eq!(fetch_one.kind, FunctionKind::AsyncFunction);
    }
    #[test]
    fn rust_self_is_not_a_local() {
        let r = run(
            "impl T { fn a(&self, x: i32) -> i32 { x } fn b(&self, y: i32) -> i32 { y } }",
            "rs",
            NormalizationLevel::Balanced,
        );
        assert_eq!(r.normalized[0].tokens, r.normalized[1].tokens);
        let r = run(
            "impl T { fn a(&self) -> i32 { self.x } fn b(&self) -> i32 { self.y } }",
            "rs",
            NormalizationLevel::Balanced,
        );
        assert_ne!(r.normalized[0].tokens, r.normalized[1].tokens);
    }
    #[test]
    fn rust_control_flow_and_macros_survive() {
        let r = run(
            "fn a(x: i32) -> i32 { if x > 0 { x } else { 0 } } fn b(x: i32) -> i32 { match x { n if n > 0 => n, _ => 0 } }",
            "rs",
            NormalizationLevel::Balanced,
        );
        assert_ne!(r.normalized[0].tokens, r.normalized[1].tokens);
        let r = run(
            "fn a(x: i32) -> i32 { foo(x) } fn b(x: i32) -> i32 { foo(x)? }",
            "rs",
            NormalizationLevel::Balanced,
        );
        assert_ne!(r.normalized[0].tokens, r.normalized[1].tokens);
        let r = run(
            "fn a() { println!(\"a\"); } fn b() { eprintln!(\"a\"); }",
            "rs",
            NormalizationLevel::Balanced,
        );
        assert_ne!(r.normalized[0].tokens, r.normalized[1].tokens);
    }

    #[test]
    fn rust_macro_token_trees_do_not_emit_syntax_errors() {
        for source in [
            r##"fn check() { snap![[r#"arbitrary macro input"#]]; }"##,
            r#"fn check() { let value = str!["1.43.0-beta.1"]; }"#,
        ] {
            let r = run(source, "rs", NormalizationLevel::Balanced);
            assert!(r.parse_errors.is_empty(), "{source}");
            assert_eq!(r.functions.len(), 1);
        }
    }

    #[test]
    fn parser_diagnostics_are_bounded_per_file() {
        let source = "int = ;\n".repeat(100);
        let r = run(&source, "c", NormalizationLevel::Balanced);
        assert_eq!(r.parse_errors.len(), MAX_PARSE_ERRORS_PER_FILE + 1);
        assert!(r.parse_errors.last().is_some_and(|error| error
            .message
            .contains("additional parser diagnostics omitted")));
    }

    #[test]
    fn rust_nested_fn_does_not_pollute_parent_calls() {
        let r = run(
            "fn outer(x: i32) -> i32 { fn inner(y: i32) -> i32 { danger(y) } safe(x) }",
            "rs",
            NormalizationLevel::Balanced,
        );
        assert_eq!(r.functions.len(), 2);
        assert_eq!(r.functions[1].qualified_name, "outer::inner");
        assert!(r.functions[0].called_functions.contains("safe"));
        assert!(!r.functions[0].called_functions.contains("danger"));
    }
    #[test]
    fn vhdl_case_and_named_endings() {
        let r = run("package body p is function First(x: integer) return integer is begin return x + 1; end function First; FUNCTION SECOND(Y: INTEGER) RETURN INTEGER IS BEGIN RETURN Y + 1; END FUNCTION SECOND; end package body;", "vhd", NormalizationLevel::Balanced);
        assert!(r.parse_errors.is_empty());
        assert_eq!(r.normalized[0].tokens, r.normalized[1].tokens);
    }

    #[test]
    fn systemverilog_uvm_headers_recover_for_discovery() {
        let source = r#"`ifndef PACKET_SVH
`define PACKET_SVH
`include "uvm_macros.svh"
import uvm_pkg::*;
class packet extends uvm_sequence_item;
  `uvm_object_utils(packet)
  function new(string name = "packet");
    super.new(name);
  endfunction
  function void do_print(uvm_printer printer);
    super.do_print(printer);
    printer.print_field("data", data, 16, UVM_HEX);
  endfunction
endclass
`endif
"#;
        let result = run(source, "svh", NormalizationLevel::Balanced);
        assert!(result.parse_errors.is_empty());
        assert_eq!(result.functions.len(), 2);
        assert_eq!(result.functions[0].qualified_name, "packet::new");
        assert_eq!(result.functions[1].qualified_name, "packet::do_print");
        assert!(result
            .functions
            .iter()
            .all(|function| function.parser_recovered));
    }

    #[test]
    fn ambiguous_headers_select_the_more_accurate_c_or_cpp_grammar() {
        let cpp = run(
            "template <typename T> T identity(T value) { return value; }",
            "h",
            NormalizationLevel::Balanced,
        );
        assert_eq!(cpp.functions[0].language, Language::Cpp);

        let c = run(
            "int identity(int value) { return value; }",
            "h",
            NormalizationLevel::Balanced,
        );
        assert_eq!(c.functions[0].language, Language::C);
    }
}
