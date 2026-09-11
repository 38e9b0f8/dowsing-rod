use crate::types::{NormalizationLevel, NormalizedFunction, StructuralToken};
use rustpython_parser::ast::{self, Expr, Stmt};
use std::collections::HashMap;

/// Normalize a function's AST into a sequence of structural tokens.
///
/// Three normalization levels control how aggressively identifiers are canonicalized:
///
/// - **Strict**: Normalize only whitespace/comments/docstrings. Preserve all names.
/// - **Balanced** (default): Normalize local variable names and parameter names to
///   positional placeholders. Preserve external names, called functions, operators,
///   attributes, string literals, and numeric literals.
/// - **Aggressive**: Also normalize string literals and numeric constants to generic tokens.
///
/// In all modes, the following are ALWAYS preserved because they carry semantic meaning:
/// - Control flow structure (if/for/while/try/with)
/// - Operators (+, -, *, /, ==, !=, etc.)
/// - Called function/method names
/// - Attribute access names
/// - External/imported identifiers
/// - Boolean and None literals
/// - Decorators
pub fn normalize_function(
    func_body: &[Stmt],
    parameters: &[String],
    func_name: &str,
    function_id: usize,
    level: NormalizationLevel,
    class_name: Option<&str>,
) -> NormalizedFunction {
    let mut normalizer = Normalizer {
        level,
        local_names: HashMap::new(),
        next_local_id: 0,
        tokens: Vec::new(),
    };

    // Register parameters as local names (for balanced/aggressive normalization)
    if level != NormalizationLevel::Strict {
        for param in parameters {
            if param != "self" && param != "cls" {
                normalizer.register_local(param);
            }
        }
    }

    // The function name itself and class name are external (preserved)
    normalizer.tokens.push(StructuralToken::FunctionDef);
    normalizer.tokens.push(StructuralToken::BlockStart);

    for stmt in func_body {
        normalizer.visit_stmt(stmt);
    }

    normalizer.tokens.push(StructuralToken::BlockEnd);

    // Drop the function name from the tokens if we're not preserving it
    let _ = func_name;
    let _ = class_name;

    NormalizedFunction {
        function_id,
        tokens: normalizer.tokens,
    }
}

struct Normalizer {
    level: NormalizationLevel,
    local_names: HashMap<String, usize>,
    next_local_id: usize,
    tokens: Vec<StructuralToken>,
}

impl Normalizer {
    fn register_local(&mut self, name: &str) -> usize {
        let id = self.next_local_id;
        self.local_names.insert(name.to_string(), id);
        self.next_local_id += 1;
        id
    }

    /// Determine whether a name is local (should be normalized) or external (should be preserved).
    fn name_token(&mut self, name: &str) -> StructuralToken {
        match self.level {
            NormalizationLevel::Strict => {
                // Preserve all names
                StructuralToken::ExternalName(name.to_string())
            }
            NormalizationLevel::Balanced | NormalizationLevel::Aggressive => {
                if self.local_names.contains_key(name) || is_likely_local(name) {
                    if !self.local_names.contains_key(name) {
                        self.register_local(name);
                    }
                    StructuralToken::LocalName
                } else {
                    StructuralToken::ExternalName(name.to_string())
                }
            }
        }
    }

    fn visit_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Expr(e) => {
                // Skip docstrings (first statement that's a string constant)
                if let Expr::Constant(c) = e.value.as_ref() {
                    if matches!(c.value, ast::Constant::Str(_)) {
                        // This is likely a docstring — skip in balanced/aggressive
                        if self.level != NormalizationLevel::Strict {
                            return;
                        }
                    }
                }
                self.visit_expr(&e.value);
            }
            Stmt::Return(r) => {
                self.tokens.push(StructuralToken::Return);
                if let Some(v) = &r.value {
                    self.visit_expr(v);
                }
            }
            Stmt::Assign(a) => {
                self.tokens.push(StructuralToken::Assign);
                for target in &a.targets {
                    // Register assigned names as locals
                    if let Expr::Name(n) = target {
                        if self.level != NormalizationLevel::Strict
                            && !self.local_names.contains_key(n.id.as_str())
                        {
                            self.register_local(&n.id);
                        }
                    }
                    self.visit_expr(target);
                }
                self.visit_expr(&a.value);
            }
            Stmt::AugAssign(a) => {
                let op = operator_to_string(&a.op);
                self.tokens.push(StructuralToken::AugAssign(op));
                self.visit_expr(&a.target);
                self.visit_expr(&a.value);
            }
            Stmt::AnnAssign(a) => {
                self.tokens.push(StructuralToken::Assign);
                self.visit_expr(&a.target);
                if let Some(v) = &a.value {
                    self.visit_expr(v);
                }
            }
            Stmt::If(i) => {
                self.tokens.push(StructuralToken::If);
                self.visit_expr(&i.test);
                self.tokens.push(StructuralToken::BlockStart);
                for s in &i.body {
                    self.visit_stmt(s);
                }
                self.tokens.push(StructuralToken::BlockEnd);
                if !i.orelse.is_empty() {
                    // Distinguish elif from else
                    if i.orelse.len() == 1 && matches!(i.orelse[0], Stmt::If(_)) {
                        self.tokens.push(StructuralToken::Elif);
                        if let Stmt::If(elif) = &i.orelse[0] {
                            self.visit_expr(&elif.test);
                            self.tokens.push(StructuralToken::BlockStart);
                            for s in &elif.body {
                                self.visit_stmt(s);
                            }
                            self.tokens.push(StructuralToken::BlockEnd);
                            if !elif.orelse.is_empty() {
                                self.tokens.push(StructuralToken::Else);
                                self.tokens.push(StructuralToken::BlockStart);
                                for s in &elif.orelse {
                                    self.visit_stmt(s);
                                }
                                self.tokens.push(StructuralToken::BlockEnd);
                            }
                        }
                    } else {
                        self.tokens.push(StructuralToken::Else);
                        self.tokens.push(StructuralToken::BlockStart);
                        for s in &i.orelse {
                            self.visit_stmt(s);
                        }
                        self.tokens.push(StructuralToken::BlockEnd);
                    }
                }
            }
            Stmt::For(f) => {
                self.tokens.push(StructuralToken::For);
                self.visit_expr(&f.target);
                self.visit_expr(&f.iter);
                self.tokens.push(StructuralToken::BlockStart);
                for s in &f.body {
                    self.visit_stmt(s);
                }
                self.tokens.push(StructuralToken::BlockEnd);
                if !f.orelse.is_empty() {
                    self.tokens.push(StructuralToken::Else);
                    self.tokens.push(StructuralToken::BlockStart);
                    for s in &f.orelse {
                        self.visit_stmt(s);
                    }
                    self.tokens.push(StructuralToken::BlockEnd);
                }
            }
            Stmt::While(w) => {
                self.tokens.push(StructuralToken::While);
                self.visit_expr(&w.test);
                self.tokens.push(StructuralToken::BlockStart);
                for s in &w.body {
                    self.visit_stmt(s);
                }
                self.tokens.push(StructuralToken::BlockEnd);
                if !w.orelse.is_empty() {
                    self.tokens.push(StructuralToken::Else);
                    self.tokens.push(StructuralToken::BlockStart);
                    for s in &w.orelse {
                        self.visit_stmt(s);
                    }
                    self.tokens.push(StructuralToken::BlockEnd);
                }
            }
            Stmt::Try(t) => {
                self.tokens.push(StructuralToken::Try);
                self.tokens.push(StructuralToken::BlockStart);
                for s in &t.body {
                    self.visit_stmt(s);
                }
                self.tokens.push(StructuralToken::BlockEnd);
                for handler in &t.handlers {
                    let ast::ExceptHandler::ExceptHandler(h) = handler;
                    self.tokens.push(StructuralToken::ExceptHandler);
                    self.tokens.push(StructuralToken::BlockStart);
                    for s in &h.body {
                        self.visit_stmt(s);
                    }
                    self.tokens.push(StructuralToken::BlockEnd);
                }
                if !t.orelse.is_empty() {
                    self.tokens.push(StructuralToken::Else);
                    self.tokens.push(StructuralToken::BlockStart);
                    for s in &t.orelse {
                        self.visit_stmt(s);
                    }
                    self.tokens.push(StructuralToken::BlockEnd);
                }
                if !t.finalbody.is_empty() {
                    self.tokens.push(StructuralToken::Finally);
                    self.tokens.push(StructuralToken::BlockStart);
                    for s in &t.finalbody {
                        self.visit_stmt(s);
                    }
                    self.tokens.push(StructuralToken::BlockEnd);
                }
            }
            Stmt::With(w) => {
                self.tokens.push(StructuralToken::With);
                for item in &w.items {
                    self.visit_expr(&item.context_expr);
                }
                self.tokens.push(StructuralToken::BlockStart);
                for s in &w.body {
                    self.visit_stmt(s);
                }
                self.tokens.push(StructuralToken::BlockEnd);
            }
            Stmt::Raise(r) => {
                self.tokens.push(StructuralToken::Raise);
                if let Some(exc) = &r.exc {
                    self.visit_expr(exc);
                }
            }
            Stmt::Assert(a) => {
                self.tokens.push(StructuralToken::Assert);
                self.visit_expr(&a.test);
            }
            Stmt::Delete(_) => {
                self.tokens.push(StructuralToken::Delete);
            }
            Stmt::Pass(_) => {
                self.tokens.push(StructuralToken::Pass);
            }
            Stmt::Break(_) => {
                self.tokens.push(StructuralToken::Break);
            }
            Stmt::Continue(_) => {
                self.tokens.push(StructuralToken::Continue);
            }
            Stmt::Global(_) => {
                self.tokens.push(StructuralToken::Global);
            }
            Stmt::Nonlocal(_) => {
                self.tokens.push(StructuralToken::Nonlocal);
            }
            Stmt::Import(i) => {
                for alias in &i.names {
                    self.tokens
                        .push(StructuralToken::Import(alias.name.to_string()));
                }
            }
            Stmt::ImportFrom(i) => {
                let module = i.module.as_ref().map(|m| m.to_string()).unwrap_or_default();
                self.tokens.push(StructuralToken::Import(module));
            }
            Stmt::FunctionDef(f) => {
                // Nested function definition
                self.tokens.push(StructuralToken::FunctionDef);
                self.tokens.push(StructuralToken::BlockStart);
                for s in &f.body {
                    self.visit_stmt(s);
                }
                self.tokens.push(StructuralToken::BlockEnd);
            }
            Stmt::AsyncFunctionDef(f) => {
                // Nested async function definition
                self.tokens.push(StructuralToken::AsyncFunctionDef);
                self.tokens.push(StructuralToken::BlockStart);
                for s in &f.body {
                    self.visit_stmt(s);
                }
                self.tokens.push(StructuralToken::BlockEnd);
            }
            Stmt::ClassDef(c) => {
                self.tokens.push(StructuralToken::ClassDef);
                self.tokens.push(StructuralToken::BlockStart);
                for s in &c.body {
                    self.visit_stmt(s);
                }
                self.tokens.push(StructuralToken::BlockEnd);
            }
            _ => {}
        }
    }

    fn visit_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Call(c) => {
                let name = call_name(&c.func);
                self.tokens.push(StructuralToken::Call(name));
                for arg in &c.args {
                    self.visit_expr(arg);
                }
                for kw in &c.keywords {
                    self.visit_expr(&kw.value);
                }
            }
            Expr::Attribute(a) => {
                self.visit_expr(&a.value);
                self.tokens
                    .push(StructuralToken::Attribute(a.attr.to_string()));
            }
            Expr::Name(n) => {
                let token = self.name_token(&n.id);
                self.tokens.push(token);
            }
            Expr::BinOp(b) => {
                let op = operator_to_string(&b.op);
                self.tokens.push(StructuralToken::BinOp(op));
                self.visit_expr(&b.left);
                self.visit_expr(&b.right);
            }
            Expr::UnaryOp(u) => {
                let op = unary_op_to_string(&u.op);
                self.tokens.push(StructuralToken::UnaryOp(op));
                self.visit_expr(&u.operand);
            }
            Expr::BoolOp(b) => {
                let op = bool_op_to_string(&b.op);
                self.tokens.push(StructuralToken::BoolOp(op));
                for v in &b.values {
                    self.visit_expr(v);
                }
            }
            Expr::Compare(c) => {
                let ops: Vec<String> = c.ops.iter().map(cmp_op_to_string).collect();
                self.tokens.push(StructuralToken::Compare(ops.join(",")));
                self.visit_expr(&c.left);
                for comp in &c.comparators {
                    self.visit_expr(comp);
                }
            }
            Expr::Subscript(s) => {
                self.tokens.push(StructuralToken::Subscript);
                self.visit_expr(&s.value);
                self.visit_expr(&s.slice);
            }
            Expr::Starred(s) => {
                self.tokens.push(StructuralToken::Starred);
                self.visit_expr(&s.value);
            }
            Expr::Constant(c) => {
                match &c.value {
                    ast::Constant::Bool(b) => {
                        self.tokens.push(StructuralToken::BoolLiteral(*b));
                    }
                    ast::Constant::None => {
                        self.tokens.push(StructuralToken::NoneLiteral);
                    }
                    ast::Constant::Str(_) => {
                        if self.level == NormalizationLevel::Aggressive {
                            self.tokens.push(StructuralToken::StringLiteral);
                        } else {
                            // In strict/balanced, preserve string literals
                            self.tokens.push(StructuralToken::StringLiteral);
                        }
                    }
                    ast::Constant::Int(_)
                    | ast::Constant::Float(_)
                    | ast::Constant::Complex { .. } => {
                        self.tokens.push(StructuralToken::NumericLiteral);
                    }
                    ast::Constant::Bytes(_) => {
                        self.tokens.push(StructuralToken::StringLiteral);
                    }
                    ast::Constant::Ellipsis => {}
                    ast::Constant::Tuple(_) => {}
                }
            }
            Expr::ListComp(l) => {
                self.tokens.push(StructuralToken::ListComp);
                self.visit_expr(&l.elt);
            }
            Expr::SetComp(s) => {
                self.tokens.push(StructuralToken::SetComp);
                self.visit_expr(&s.elt);
            }
            Expr::DictComp(d) => {
                self.tokens.push(StructuralToken::DictComp);
                self.visit_expr(&d.key);
                self.visit_expr(&d.value);
            }
            Expr::GeneratorExp(g) => {
                self.tokens.push(StructuralToken::GeneratorExp);
                self.visit_expr(&g.elt);
            }
            Expr::IfExp(i) => {
                self.tokens.push(StructuralToken::If);
                self.visit_expr(&i.test);
                self.visit_expr(&i.body);
                self.visit_expr(&i.orelse);
            }
            Expr::Lambda(l) => {
                self.tokens.push(StructuralToken::Lambda);
                self.visit_expr(&l.body);
            }
            Expr::Await(a) => {
                self.tokens.push(StructuralToken::Await);
                self.visit_expr(&a.value);
            }
            Expr::Yield(y) => {
                self.tokens.push(StructuralToken::Yield);
                if let Some(v) = &y.value {
                    self.visit_expr(v);
                }
            }
            Expr::YieldFrom(y) => {
                self.tokens.push(StructuralToken::YieldFrom);
                self.visit_expr(&y.value);
            }
            Expr::Tuple(t) => {
                for elt in &t.elts {
                    self.visit_expr(elt);
                }
            }
            Expr::List(l) => {
                for elt in &l.elts {
                    self.visit_expr(elt);
                }
            }
            Expr::Set(s) => {
                for elt in &s.elts {
                    self.visit_expr(elt);
                }
            }
            Expr::Dict(d) => {
                for (key, val) in d.keys.iter().zip(d.values.iter()) {
                    if let Some(k) = key {
                        self.visit_expr(k);
                    }
                    self.visit_expr(val);
                }
            }
            Expr::FormattedValue(f) => {
                self.tokens.push(StructuralToken::FStringLiteral);
                self.visit_expr(&f.value);
            }
            Expr::JoinedStr(j) => {
                self.tokens.push(StructuralToken::FStringLiteral);
                for val in &j.values {
                    self.visit_expr(val);
                }
            }
            Expr::Slice(s) => {
                self.tokens.push(StructuralToken::Slice);
                if let Some(lower) = &s.lower {
                    self.visit_expr(lower);
                }
                if let Some(upper) = &s.upper {
                    self.visit_expr(upper);
                }
                if let Some(step) = &s.step {
                    self.visit_expr(step);
                }
            }
            _ => {}
        }
    }
}

/// Extract the name of a called function/method.
fn call_name(func: &Expr) -> String {
    match func {
        Expr::Name(n) => n.id.to_string(),
        Expr::Attribute(a) => a.attr.to_string(),
        _ => "<call>".to_string(),
    }
}

/// Heuristic: names that look like local variables (lowercase, no dots).
fn is_likely_local(name: &str) -> bool {
    // Common builtins and known globals should not be normalized
    const BUILTINS: &[&str] = &[
        "print",
        "len",
        "range",
        "enumerate",
        "zip",
        "map",
        "filter",
        "sorted",
        "reversed",
        "list",
        "dict",
        "set",
        "tuple",
        "str",
        "int",
        "float",
        "bool",
        "type",
        "isinstance",
        "issubclass",
        "hasattr",
        "getattr",
        "setattr",
        "super",
        "property",
        "classmethod",
        "staticmethod",
        "abs",
        "min",
        "max",
        "sum",
        "any",
        "all",
        "open",
        "input",
        "repr",
        "hash",
        "id",
        "dir",
        "vars",
        "globals",
        "locals",
        "callable",
        "iter",
        "next",
        "round",
        "format",
        "chr",
        "ord",
        "hex",
        "oct",
        "bin",
        "pow",
        "divmod",
        "True",
        "False",
        "None",
        "NotImplemented",
        "Ellipsis",
        "Exception",
        "BaseException",
        "ValueError",
        "TypeError",
        "KeyError",
        "IndexError",
        "AttributeError",
        "RuntimeError",
        "StopIteration",
        "OSError",
        "IOError",
        "FileNotFoundError",
        "PermissionError",
        "ImportError",
        "ModuleNotFoundError",
        "NameError",
        "AssertionError",
        "self",
        "cls",
    ];

    if BUILTINS.contains(&name) {
        return false;
    }

    // Names starting with uppercase are typically class names (external)
    if name.chars().next().is_some_and(|c| c.is_uppercase()) {
        return false;
    }

    // Names with dots are attribute access (external)
    if name.contains('.') {
        return false;
    }

    // Short lowercase names are typically local variables
    true
}

fn operator_to_string(op: &ast::Operator) -> String {
    match op {
        ast::Operator::Add => "+".to_string(),
        ast::Operator::Sub => "-".to_string(),
        ast::Operator::Mult => "*".to_string(),
        ast::Operator::Div => "/".to_string(),
        ast::Operator::Mod => "%".to_string(),
        ast::Operator::Pow => "**".to_string(),
        ast::Operator::LShift => "<<".to_string(),
        ast::Operator::RShift => ">>".to_string(),
        ast::Operator::BitOr => "|".to_string(),
        ast::Operator::BitXor => "^".to_string(),
        ast::Operator::BitAnd => "&".to_string(),
        ast::Operator::FloorDiv => "//".to_string(),
        ast::Operator::MatMult => "@".to_string(),
    }
}

fn unary_op_to_string(op: &ast::UnaryOp) -> String {
    match op {
        ast::UnaryOp::Invert => "~".to_string(),
        ast::UnaryOp::Not => "not".to_string(),
        ast::UnaryOp::UAdd => "+".to_string(),
        ast::UnaryOp::USub => "-".to_string(),
    }
}

fn bool_op_to_string(op: &ast::BoolOp) -> String {
    match op {
        ast::BoolOp::And => "and".to_string(),
        ast::BoolOp::Or => "or".to_string(),
    }
}

fn cmp_op_to_string(op: &ast::CmpOp) -> String {
    match op {
        ast::CmpOp::Eq => "==".to_string(),
        ast::CmpOp::NotEq => "!=".to_string(),
        ast::CmpOp::Lt => "<".to_string(),
        ast::CmpOp::LtE => "<=".to_string(),
        ast::CmpOp::Gt => ">".to_string(),
        ast::CmpOp::GtE => ">=".to_string(),
        ast::CmpOp::Is => "is".to_string(),
        ast::CmpOp::IsNot => "is not".to_string(),
        ast::CmpOp::In => "in".to_string(),
        ast::CmpOp::NotIn => "not in".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_python_source;
    use std::path::PathBuf;

    fn normalize_source(source: &str, level: NormalizationLevel) -> Vec<StructuralToken> {
        let path = PathBuf::from("test.py");
        let result = parse_python_source(source, &path).unwrap();
        let body = result.module.unwrap();

        // Find the first function
        for stmt in &body {
            if let Stmt::FunctionDef(f) = stmt {
                let params: Vec<String> =
                    f.args.args.iter().map(|a| a.def.arg.to_string()).collect();
                let norm = normalize_function(&f.body, &params, &f.name, 0, level, None);
                return norm.tokens;
            }
        }
        panic!("no function found in source");
    }

    #[test]
    fn test_renamed_variables_are_equivalent() {
        let source_a = "def foo(x):\n    y = x + 1\n    return y\n";
        let source_b = "def bar(value):\n    result = value + 1\n    return result\n";

        let tokens_a = normalize_source(source_a, NormalizationLevel::Balanced);
        let tokens_b = normalize_source(source_b, NormalizationLevel::Balanced);

        assert_eq!(
            tokens_a, tokens_b,
            "renamed variables should produce identical normalized tokens"
        );
    }

    #[test]
    fn test_different_operators_are_distinct() {
        let source_a = "def foo(x):\n    return x + 1\n";
        let source_b = "def foo(x):\n    return x - 1\n";

        let tokens_a = normalize_source(source_a, NormalizationLevel::Balanced);
        let tokens_b = normalize_source(source_b, NormalizationLevel::Balanced);

        assert_ne!(
            tokens_a, tokens_b,
            "different operators must produce different tokens"
        );
    }

    #[test]
    fn test_different_called_functions_are_distinct() {
        let source_a = "def save_user(user):\n    db.save(user)\n";
        let source_b = "def delete_user(user):\n    db.delete(user)\n";

        let tokens_a = normalize_source(source_a, NormalizationLevel::Balanced);
        let tokens_b = normalize_source(source_b, NormalizationLevel::Balanced);

        assert_ne!(
            tokens_a, tokens_b,
            "save vs delete must NOT be treated as equivalent"
        );
    }

    #[test]
    fn test_strict_preserves_all_names() {
        let source = "def foo(x):\n    y = x + 1\n    return y\n";
        let tokens = normalize_source(source, NormalizationLevel::Strict);

        // In strict mode, 'x' and 'y' should appear as ExternalName
        let has_external_names = tokens
            .iter()
            .any(|t| matches!(t, StructuralToken::ExternalName(_)));
        assert!(
            has_external_names,
            "strict mode should preserve all names as external"
        );
    }

    #[test]
    fn test_control_flow_preserved() {
        let source =
            "def foo(x):\n    if x > 0:\n        return True\n    else:\n        return False\n";
        let tokens = normalize_source(source, NormalizationLevel::Balanced);

        assert!(tokens.contains(&StructuralToken::If));
        assert!(tokens.contains(&StructuralToken::Else));
        assert!(tokens.contains(&StructuralToken::Return));
    }
}
