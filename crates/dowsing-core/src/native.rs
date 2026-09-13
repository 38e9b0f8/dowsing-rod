//! Tree-sitter frontends. Grammars are compiled into the binary; source stays local.
use crate::{language::Language, types::*, FileAnalysis};
use anyhow::{Context, Result};
use std::{
    collections::{BTreeSet, HashMap},
    path::Path,
};
use tree_sitter::{Node, Parser, Tree};

pub(crate) fn analyze(
    source: &str,
    path: &Path,
    language: Language,
    level: NormalizationLevel,
) -> Result<FileAnalysis> {
    let (language, tree) = select_grammar(source, path, language)?;
    let mut result = FileAnalysis::default();
    let mut pending = vec![tree.root_node()];
    while let Some(node) = pending.pop() {
        if node.is_error() || node.is_missing() {
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
            // One diagnostic per erroneous region, without floods from its children.
            continue;
        }
        if let Some(kind) = unit_kind(node, language) {
            // Error recovery can manufacture plausible functions. Only analyze intact units.
            if !node.has_error() {
                let (info, normalized) = extract(
                    node,
                    source,
                    path,
                    language,
                    kind,
                    level,
                    result.functions.len(),
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
    Ok(result)
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

fn parameter_nodes(node: Node<'_>, language: Language) -> Vec<Node<'_>> {
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
                    parameters.push(name);
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
            let mut cursor = n.walk();
            let children: Vec<_> = n.named_children(&mut cursor).collect();
            pending.extend(children.into_iter().rev());
        }
    }
    parameters
}

fn extract(
    node: Node<'_>,
    source: &str,
    path: &Path,
    language: Language,
    mut kind: FunctionKind,
    level: NormalizationLevel,
    id: usize,
) -> (FunctionInfo, NormalizedFunction) {
    let name_node = if kind == FunctionKind::Process && language != Language::Vhdl {
        None
    } else {
        unit_name(node, language)
    };
    let name = name_node
        .map(|n| text(n, source).trim_end_matches(':').trim().to_string())
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
    if language == Language::Cpp && kind == FunctionKind::Function {
        let mut parent = node.parent();
        while let Some(p) = parent {
            if matches!(p.kind(), "class_specifier" | "struct_specifier") {
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
        ) || unit_kind(p, language).is_some()
        {
            let scope = p
                .child_by_field_name("name")
                .or_else(|| p.child_by_field_name("architecture"))
                .or_else(|| {
                    child_kind(p, "module_header")
                        .or_else(|| child_kind(p, "module_nonansi_header"))
                        .or_else(|| child_kind(p, "module_ansi_header"))
                        .and_then(|n| find_kind(n, &["simple_identifier", "escaped_identifier"]))
                })
                .or_else(|| unit_name(p, language));
            if let Some(scope) = scope {
                let scope = text(scope, source).to_string();
                if class_name.is_none() && p.kind().contains("class") {
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
    let parameters: Vec<String> = parameter_nodes(node, language)
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
        ) && p
            .child_by_field_name("expression")
            .or_else(|| p.child_by_field_name("object"))
            .or_else(|| p.child_by_field_name("argument"))
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
            | "character_literal"
            | "char_literal"
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
        | "if_else_statement" => If,
        "for_statement" | "for_in_statement" | "for_range_loop" | "foreach_statement"
        | "loop_statement" => For,
        "while_statement" | "do_statement" => While,
        "switch_statement" | "case_statement" => Match,
        "case_item" | "case_statement_alternative" | "switch_section" => Case,
        "return_statement" => Return,
        "break_statement" => Break,
        "continue_statement" => Continue,
        "try_statement" => Try,
        "catch_clause" => ExceptHandler,
        "throw_statement" => Raise,
        "else" if value == "else" => Else,
        _ => return None,
    })
}

fn call_target(node: Node<'_>, source: &str, language: Language) -> Option<String> {
    let target = match node.kind() {
        "call_expression" => node.child_by_field_name("function"),
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
            ("ts", "function good(x: number) { return x; } function broken( {"),
            ("c", "int good(int x) { return x; } int broken( {"),
            ("cpp", "int good(int x) { return x; } int broken( {"),
            ("cs", "class A { int Good(int x) { return x; } int Broken( { }"),
            ("sv", "module m; function int good(int x); return x; endfunction always_ff @( begin endmodule"),
            ("vhd", "package body p is function good(x: integer) return integer is begin return x; end function; function broken( end package body;"),
        ] {
            let r = run(source, ext, NormalizationLevel::Balanced);
            assert!(!r.parse_errors.is_empty(), "{ext}");
            assert!(r.parse_errors.iter().all(|e| e.line.is_some() && e.column.is_some()));
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
    fn nested_bodies_do_not_pollute_parent_calls() {
        let r = run("function outer(x: number) { function inner(y: number) { return danger(y); } return safe(x); }", "ts", NormalizationLevel::Balanced);
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
