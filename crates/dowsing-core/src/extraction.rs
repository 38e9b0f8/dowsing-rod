use crate::types::{FunctionInfo, FunctionKind};
use rustpython_parser::ast::{self, Expr, Stmt};
use rustpython_parser::source_code::RandomLocator;
use std::collections::BTreeSet;
use std::path::Path;

/// Extract all functions and methods from a parsed Python AST.
pub fn extract_functions(stmts: &[ast::Stmt], source: &str, file_path: &Path) -> Vec<FunctionInfo> {
    let mut functions = Vec::new();
    let mut extractor = FunctionExtractor {
        source,
        file_path,
        functions: &mut functions,
        class_stack: Vec::new(),
        nesting_depth: 0,
    };
    extractor.visit_body(stmts);
    functions
}

struct FunctionExtractor<'a> {
    source: &'a str,
    file_path: &'a Path,
    functions: &'a mut Vec<FunctionInfo>,
    class_stack: Vec<String>,
    nesting_depth: usize,
}

enum FunctionDefRef<'a> {
    Sync(&'a ast::StmtFunctionDef),
    Async(&'a ast::StmtAsyncFunctionDef),
}

impl FunctionDefRef<'_> {
    fn is_async(&self) -> bool {
        matches!(self, Self::Async(_))
    }

    fn name(&self) -> &str {
        match self {
            Self::Sync(f) => f.name.as_str(),
            Self::Async(f) => f.name.as_str(),
        }
    }

    fn args(&self) -> &ast::Arguments {
        match self {
            Self::Sync(f) => f.args.as_ref(),
            Self::Async(f) => f.args.as_ref(),
        }
    }

    fn body(&self) -> &[Stmt] {
        match self {
            Self::Sync(f) => &f.body,
            Self::Async(f) => &f.body,
        }
    }

    fn decorators(&self) -> &[Expr] {
        match self {
            Self::Sync(f) => &f.decorator_list,
            Self::Async(f) => &f.decorator_list,
        }
    }

    fn returns(&self) -> Option<&Expr> {
        match self {
            Self::Sync(f) => f.returns.as_deref(),
            Self::Async(f) => f.returns.as_deref(),
        }
    }

    fn start_line(&self, source: &str) -> usize {
        let offset = match self {
            Self::Sync(f) => f.range.start(),
            Self::Async(f) => f.range.start(),
        };
        line_for_offset(source, offset)
    }

    fn end_line(&self, source: &str) -> usize {
        let offset = match self {
            Self::Sync(f) => f.range.end(),
            Self::Async(f) => f.range.end(),
        };
        line_for_offset(source, offset)
    }
}

impl<'a> FunctionExtractor<'a> {
    fn visit_body(&mut self, stmts: &[Stmt]) {
        for stmt in stmts {
            self.visit_stmt(stmt);
        }
    }

    fn visit_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::FunctionDef(func_def) => {
                self.extract_function(FunctionDefRef::Sync(func_def));
            }
            Stmt::AsyncFunctionDef(func_def) => {
                self.extract_function(FunctionDefRef::Async(func_def));
            }
            Stmt::ClassDef(class_def) => {
                self.class_stack.push(class_def.name.to_string());
                self.visit_body(&class_def.body);
                self.class_stack.pop();
            }
            Stmt::If(if_stmt) => {
                self.visit_body(&if_stmt.body);
                self.visit_body(&if_stmt.orelse);
            }
            Stmt::For(for_stmt) => {
                self.visit_body(&for_stmt.body);
                self.visit_body(&for_stmt.orelse);
            }
            Stmt::While(while_stmt) => {
                self.visit_body(&while_stmt.body);
                self.visit_body(&while_stmt.orelse);
            }
            Stmt::Try(try_stmt) => {
                self.visit_body(&try_stmt.body);
                for handler in &try_stmt.handlers {
                    let ast::ExceptHandler::ExceptHandler(h) = handler;
                    self.visit_body(&h.body);
                }
                self.visit_body(&try_stmt.orelse);
                self.visit_body(&try_stmt.finalbody);
            }
            Stmt::With(with_stmt) => {
                self.visit_body(&with_stmt.body);
            }
            _ => {}
        }
    }

    fn extract_function(&mut self, func_def: FunctionDefRef<'_>) {
        let name = func_def.name().to_string();
        let is_async = func_def.is_async();
        let in_class = !self.class_stack.is_empty();
        let is_nested = self.nesting_depth > 0;

        // Determine function kind
        let kind = match (is_async, in_class, is_nested) {
            (true, true, _) => FunctionKind::AsyncMethod,
            (true, false, _) => FunctionKind::AsyncFunction,
            (false, true, _) => FunctionKind::Method,
            (false, false, true) => FunctionKind::NestedFunction,
            (false, false, false) => FunctionKind::Function,
        };

        // Build qualified name
        let qualified_name = if self.class_stack.is_empty() {
            name.clone()
        } else {
            format!("{}.{}", self.class_stack.join("."), name)
        };

        // Module name from file path
        let module = self
            .file_path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();

        // Extract decorators
        let decorators: Vec<String> = func_def.decorators().iter().map(expr_to_name).collect();

        // Check decorator-based classifications
        let is_property = decorators
            .iter()
            .any(|d| d == "property" || d.ends_with(".setter") || d.ends_with(".deleter"));
        let is_classmethod = decorators.iter().any(|d| d == "classmethod");
        let is_staticmethod = decorators.iter().any(|d| d == "staticmethod");

        // Extract parameters
        let parameters: Vec<String> = extract_parameters(func_def.args());

        // Extract return annotation
        let return_annotation = func_def.returns().map(expr_to_name);

        // Extract called functions from body
        let mut called_functions = BTreeSet::new();
        collect_calls(func_def.body(), &mut called_functions);

        // Compute source range
        let start_line = func_def.start_line(self.source);
        let end_line = func_def.end_line(self.source);

        // Approximate source bytes from line range
        let source_bytes = estimate_source_bytes(self.source, start_line, end_line);

        // Count AST nodes
        let ast_node_count = count_ast_nodes_stmts(func_def.body());

        // Compute cyclomatic complexity
        let complexity = compute_complexity(func_def.body());

        // Classification flags
        let is_public = !name.starts_with('_') || name.starts_with("__") && name.ends_with("__");
        let is_dunder = name.starts_with("__") && name.ends_with("__") && name.len() > 4;
        let is_test = name.starts_with("test_") || name.starts_with("test");

        let info = FunctionInfo {
            language: crate::language::Language::Python,
            file: self.file_path.to_path_buf(),
            module,
            qualified_name,
            class_name: self.class_stack.last().cloned(),
            function_name: name,
            kind,
            start_line,
            end_line,
            source_bytes,
            ast_node_count,
            complexity,
            decorators,
            parameters,
            return_annotation,
            called_functions,
            is_public,
            is_dunder,
            is_test,
            is_property,
            is_classmethod,
            is_staticmethod,
            is_async,
            parser_recovered: false,
        };

        self.functions.push(info);

        // Visit nested functions
        self.nesting_depth += 1;
        self.visit_body(func_def.body());
        self.nesting_depth -= 1;
    }
}

/// Extract parameter names from function arguments.
fn extract_parameters(args: &ast::Arguments) -> Vec<String> {
    let mut params = Vec::new();

    for arg in &args.posonlyargs {
        params.push(arg.def.arg.to_string());
    }
    for arg in &args.args {
        params.push(arg.def.arg.to_string());
    }
    if let Some(vararg) = &args.vararg {
        params.push(format!("*{}", vararg.arg));
    }
    for arg in &args.kwonlyargs {
        params.push(arg.def.arg.to_string());
    }
    if let Some(kwarg) = &args.kwarg {
        params.push(format!("**{}", kwarg.arg));
    }

    params
}

/// Convert an expression to a readable name string.
fn expr_to_name(expr: &Expr) -> String {
    match expr {
        Expr::Name(n) => n.id.to_string(),
        Expr::Attribute(a) => {
            format!("{}.{}", expr_to_name(&a.value), a.attr)
        }
        Expr::Call(c) => expr_to_name(&c.func),
        Expr::Subscript(s) => {
            format!("{}[...]", expr_to_name(&s.value))
        }
        Expr::Constant(c) => format!("{:?}", c.value),
        _ => "<expr>".to_string(),
    }
}

/// Recursively collect called function names from statements.
fn collect_calls(stmts: &[Stmt], calls: &mut BTreeSet<String>) {
    for stmt in stmts {
        collect_calls_stmt(stmt, calls);
    }
}

fn collect_calls_stmt(stmt: &Stmt, calls: &mut BTreeSet<String>) {
    match stmt {
        Stmt::Expr(e) => collect_calls_expr(&e.value, calls),
        Stmt::Return(r) => {
            if let Some(v) = &r.value {
                collect_calls_expr(v, calls);
            }
        }
        Stmt::Assign(a) => {
            collect_calls_expr(&a.value, calls);
        }
        Stmt::AugAssign(a) => {
            collect_calls_expr(&a.value, calls);
        }
        Stmt::AnnAssign(a) => {
            if let Some(v) = &a.value {
                collect_calls_expr(v, calls);
            }
        }
        Stmt::If(i) => {
            collect_calls_expr(&i.test, calls);
            collect_calls(&i.body, calls);
            collect_calls(&i.orelse, calls);
        }
        Stmt::For(f) => {
            collect_calls_expr(&f.iter, calls);
            collect_calls(&f.body, calls);
            collect_calls(&f.orelse, calls);
        }
        Stmt::While(w) => {
            collect_calls_expr(&w.test, calls);
            collect_calls(&w.body, calls);
            collect_calls(&w.orelse, calls);
        }
        Stmt::Try(t) => {
            collect_calls(&t.body, calls);
            for handler in &t.handlers {
                let ast::ExceptHandler::ExceptHandler(h) = handler;
                collect_calls(&h.body, calls);
            }
            collect_calls(&t.orelse, calls);
            collect_calls(&t.finalbody, calls);
        }
        Stmt::With(w) => {
            for item in &w.items {
                collect_calls_expr(&item.context_expr, calls);
            }
            collect_calls(&w.body, calls);
        }
        Stmt::FunctionDef(_) | Stmt::AsyncFunctionDef(_) => {
            // Don't descend into nested function definitions for call collection
        }
        Stmt::ClassDef(_) => {
            // Don't descend into nested class definitions
        }
        Stmt::Raise(r) => {
            if let Some(exc) = &r.exc {
                collect_calls_expr(exc, calls);
            }
        }
        Stmt::Assert(a) => {
            collect_calls_expr(&a.test, calls);
            if let Some(msg) = &a.msg {
                collect_calls_expr(msg, calls);
            }
        }
        Stmt::Delete(d) => {
            for target in &d.targets {
                collect_calls_expr(target, calls);
            }
        }
        _ => {}
    }
}

fn collect_calls_expr(expr: &Expr, calls: &mut BTreeSet<String>) {
    match expr {
        Expr::Call(c) => {
            let name = expr_to_name(&c.func);
            if name != "<expr>" {
                calls.insert(name);
            }
            collect_calls_expr(&c.func, calls);
            for arg in &c.args {
                collect_calls_expr(arg, calls);
            }
            for kw in &c.keywords {
                collect_calls_expr(&kw.value, calls);
            }
        }
        Expr::BoolOp(b) => {
            for v in &b.values {
                collect_calls_expr(v, calls);
            }
        }
        Expr::BinOp(b) => {
            collect_calls_expr(&b.left, calls);
            collect_calls_expr(&b.right, calls);
        }
        Expr::UnaryOp(u) => {
            collect_calls_expr(&u.operand, calls);
        }
        Expr::IfExp(i) => {
            collect_calls_expr(&i.test, calls);
            collect_calls_expr(&i.body, calls);
            collect_calls_expr(&i.orelse, calls);
        }
        Expr::Compare(c) => {
            collect_calls_expr(&c.left, calls);
            for comp in &c.comparators {
                collect_calls_expr(comp, calls);
            }
        }
        Expr::Attribute(a) => {
            collect_calls_expr(&a.value, calls);
        }
        Expr::Subscript(s) => {
            collect_calls_expr(&s.value, calls);
            collect_calls_expr(&s.slice, calls);
        }
        Expr::Starred(s) => {
            collect_calls_expr(&s.value, calls);
        }
        Expr::List(l) => {
            for elt in &l.elts {
                collect_calls_expr(elt, calls);
            }
        }
        Expr::Tuple(t) => {
            for elt in &t.elts {
                collect_calls_expr(elt, calls);
            }
        }
        Expr::Set(s) => {
            for elt in &s.elts {
                collect_calls_expr(elt, calls);
            }
        }
        Expr::Dict(d) => {
            for k in d.keys.iter().flatten() {
                collect_calls_expr(k, calls);
            }
            for val in &d.values {
                collect_calls_expr(val, calls);
            }
        }
        Expr::ListComp(l) => {
            collect_calls_expr(&l.elt, calls);
        }
        Expr::SetComp(s) => {
            collect_calls_expr(&s.elt, calls);
        }
        Expr::DictComp(d) => {
            collect_calls_expr(&d.key, calls);
            collect_calls_expr(&d.value, calls);
        }
        Expr::GeneratorExp(g) => {
            collect_calls_expr(&g.elt, calls);
        }
        Expr::Await(a) => {
            collect_calls_expr(&a.value, calls);
        }
        Expr::Yield(y) => {
            if let Some(v) = &y.value {
                collect_calls_expr(v, calls);
            }
        }
        Expr::YieldFrom(y) => {
            collect_calls_expr(&y.value, calls);
        }
        Expr::Lambda(l) => {
            collect_calls_expr(&l.body, calls);
        }
        Expr::FormattedValue(f) => {
            collect_calls_expr(&f.value, calls);
        }
        Expr::JoinedStr(j) => {
            for val in &j.values {
                collect_calls_expr(val, calls);
            }
        }
        _ => {}
    }
}

/// Estimate source bytes for a line range within source code.
fn estimate_source_bytes(source: &str, start_line: usize, end_line: usize) -> usize {
    let lines: Vec<&str> = source.lines().collect();
    let start_idx = start_line.saturating_sub(1).min(lines.len());
    let end_idx = end_line.min(lines.len());
    lines[start_idx..end_idx]
        .iter()
        .map(|l| l.len() + 1) // +1 for newline
        .sum()
}

fn line_for_offset(source: &str, offset: rustpython_parser::text_size::TextSize) -> usize {
    let mut locator = RandomLocator::new(source);
    locator.locate(offset).row.to_usize()
}

/// Count AST nodes in a list of statements (rough measure of complexity).
fn count_ast_nodes_stmts(stmts: &[Stmt]) -> usize {
    let mut count = 0;
    for stmt in stmts {
        count += count_ast_nodes_stmt(stmt);
    }
    count
}

fn count_ast_nodes_stmt(stmt: &Stmt) -> usize {
    let mut count = 1; // this statement itself
    match stmt {
        Stmt::FunctionDef(f) => {
            count += count_ast_nodes_stmts(&f.body);
        }
        Stmt::AsyncFunctionDef(f) => {
            count += count_ast_nodes_stmts(&f.body);
        }
        Stmt::ClassDef(c) => {
            count += count_ast_nodes_stmts(&c.body);
        }
        Stmt::If(i) => {
            count += count_ast_nodes_stmts(&i.body);
            count += count_ast_nodes_stmts(&i.orelse);
        }
        Stmt::For(f) => {
            count += count_ast_nodes_stmts(&f.body);
            count += count_ast_nodes_stmts(&f.orelse);
        }
        Stmt::While(w) => {
            count += count_ast_nodes_stmts(&w.body);
            count += count_ast_nodes_stmts(&w.orelse);
        }
        Stmt::Try(t) => {
            count += count_ast_nodes_stmts(&t.body);
            for handler in &t.handlers {
                let ast::ExceptHandler::ExceptHandler(h) = handler;
                count += 1 + count_ast_nodes_stmts(&h.body);
            }
            count += count_ast_nodes_stmts(&t.orelse);
            count += count_ast_nodes_stmts(&t.finalbody);
        }
        Stmt::With(w) => {
            count += count_ast_nodes_stmts(&w.body);
        }
        _ => {}
    }
    count
}

/// Compute cyclomatic complexity of a function body.
/// Each decision point (if, for, while, except, and, or, elif) adds 1.
fn compute_complexity(stmts: &[Stmt]) -> usize {
    let mut complexity = 1; // base complexity
    for stmt in stmts {
        complexity += complexity_stmt(stmt);
    }
    complexity
}

fn complexity_stmt(stmt: &Stmt) -> usize {
    let mut c = 0;
    match stmt {
        Stmt::If(i) => {
            c += 1; // the if itself
            c += complexity_expr(&i.test);
            for s in &i.body {
                c += complexity_stmt(s);
            }
            for s in &i.orelse {
                c += complexity_stmt(s);
            }
        }
        Stmt::For(f) => {
            c += 1;
            for s in &f.body {
                c += complexity_stmt(s);
            }
            for s in &f.orelse {
                c += complexity_stmt(s);
            }
        }
        Stmt::While(w) => {
            c += 1;
            c += complexity_expr(&w.test);
            for s in &w.body {
                c += complexity_stmt(s);
            }
            for s in &w.orelse {
                c += complexity_stmt(s);
            }
        }
        Stmt::Try(t) => {
            for handler in &t.handlers {
                let ast::ExceptHandler::ExceptHandler(h) = handler;
                c += 1;
                for s in &h.body {
                    c += complexity_stmt(s);
                }
            }
            for s in &t.body {
                c += complexity_stmt(s);
            }
            for s in &t.orelse {
                c += complexity_stmt(s);
            }
            for s in &t.finalbody {
                c += complexity_stmt(s);
            }
        }
        Stmt::With(w) => {
            for s in &w.body {
                c += complexity_stmt(s);
            }
        }
        Stmt::FunctionDef(f) => {
            for s in &f.body {
                c += complexity_stmt(s);
            }
        }
        Stmt::AsyncFunctionDef(f) => {
            for s in &f.body {
                c += complexity_stmt(s);
            }
        }
        Stmt::Assert(_) => {
            c += 1;
        }
        _ => {}
    }
    c
}

fn complexity_expr(expr: &Expr) -> usize {
    match expr {
        Expr::BoolOp(b) => {
            // Each `and`/`or` adds a decision point
            b.values.len().saturating_sub(1)
        }
        Expr::IfExp(_) => 1,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_python_source;
    use std::path::PathBuf;

    fn extract_from_source(source: &str) -> Vec<FunctionInfo> {
        let path = PathBuf::from("test.py");
        let result = parse_python_source(source, &path).unwrap();
        let body = result.module.unwrap();
        extract_functions(&body, source, &path)
    }

    #[test]
    fn test_extract_simple_function() {
        let funcs = extract_from_source("def foo(x, y):\n    return x + y\n");
        assert_eq!(funcs.len(), 1);
        assert_eq!(funcs[0].function_name, "foo");
        assert_eq!(funcs[0].parameters, vec!["x", "y"]);
        assert!(matches!(funcs[0].kind, FunctionKind::Function));
        assert!(funcs[0].is_public);
    }

    #[test]
    fn test_extract_method() {
        let source = "class Foo:\n    def bar(self):\n        pass\n";
        let funcs = extract_from_source(source);
        assert_eq!(funcs.len(), 1);
        assert_eq!(funcs[0].function_name, "bar");
        assert_eq!(funcs[0].qualified_name, "Foo.bar");
        assert_eq!(funcs[0].class_name, Some("Foo".to_string()));
        assert!(matches!(funcs[0].kind, FunctionKind::Method));
    }

    #[test]
    fn test_extract_async_function() {
        let funcs = extract_from_source("async def fetch(url):\n    pass\n");
        assert_eq!(funcs.len(), 1);
        assert!(funcs[0].is_async);
        assert!(matches!(funcs[0].kind, FunctionKind::AsyncFunction));
    }

    #[test]
    fn test_extract_private_function() {
        let funcs = extract_from_source("def _helper():\n    pass\n");
        assert_eq!(funcs.len(), 1);
        assert!(!funcs[0].is_public);
        assert!(!funcs[0].is_dunder);
    }

    #[test]
    fn test_extract_dunder() {
        let source = "class Foo:\n    def __init__(self):\n        pass\n";
        let funcs = extract_from_source(source);
        assert_eq!(funcs.len(), 1);
        assert!(funcs[0].is_dunder);
        assert!(funcs[0].is_public); // dunders are considered public
    }

    #[test]
    fn test_extract_test_function() {
        let funcs = extract_from_source("def test_something():\n    assert True\n");
        assert_eq!(funcs.len(), 1);
        assert!(funcs[0].is_test);
    }

    #[test]
    fn test_extract_decorators() {
        let source = "class C:\n    @property\n    def value(self):\n        return 1\n";
        let funcs = extract_from_source(source);
        assert_eq!(funcs.len(), 1);
        assert!(funcs[0].is_property);
        assert!(funcs[0].decorators.contains(&"property".to_string()));
    }

    #[test]
    fn test_extract_called_functions() {
        let source =
            "def process():\n    x = validate(data)\n    result = transform(x)\n    save(result)\n";
        let funcs = extract_from_source(source);
        assert_eq!(funcs.len(), 1);
        assert!(funcs[0].called_functions.contains("validate"));
        assert!(funcs[0].called_functions.contains("transform"));
        assert!(funcs[0].called_functions.contains("save"));
    }

    #[test]
    fn test_complexity() {
        let source = "def complex_func(x):\n    if x > 0:\n        for i in range(x):\n            if i % 2 == 0:\n                pass\n    else:\n        while x < 0:\n            x += 1\n";
        let funcs = extract_from_source(source);
        assert!(funcs[0].complexity >= 4); // base 1 + if + for + if + while
    }

    #[test]
    fn test_nested_functions() {
        let source = "def outer():\n    def inner():\n        pass\n    inner()\n";
        let funcs = extract_from_source(source);
        assert_eq!(funcs.len(), 2);
        assert_eq!(funcs[0].function_name, "outer");
        assert_eq!(funcs[1].function_name, "inner");
        assert!(matches!(funcs[1].kind, FunctionKind::NestedFunction));
    }

    #[test]
    fn test_staticmethod_classmethod() {
        let source = "class C:\n    @staticmethod\n    def s():\n        pass\n    @classmethod\n    def c(cls):\n        pass\n";
        let funcs = extract_from_source(source);
        assert_eq!(funcs.len(), 2);
        assert!(funcs[0].is_staticmethod);
        assert!(funcs[1].is_classmethod);
    }
}
