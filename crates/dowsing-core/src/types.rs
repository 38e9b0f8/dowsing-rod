use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

// ─── Configuration ───────────────────────────────────────────────────────

/// Normalization level controls how aggressively identifiers are canonicalized.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NormalizationLevel {
    /// Normalize only whitespace, comments, docstrings. Preserve all names.
    Strict,
    /// Normalize local/parameter names to positional placeholders.
    /// Preserve external names, called functions, operators, attributes, literals.
    #[default]
    Balanced,
    /// Also normalize string literals and numeric constants.
    Aggressive,
}

impl std::fmt::Display for NormalizationLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Strict => write!(f, "strict"),
            Self::Balanced => write!(f, "balanced"),
            Self::Aggressive => write!(f, "aggressive"),
        }
    }
}

impl std::str::FromStr for NormalizationLevel {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "strict" => Ok(Self::Strict),
            "balanced" => Ok(Self::Balanced),
            "aggressive" => Ok(Self::Aggressive),
            _ => Err(format!("unknown normalization level: {s}")),
        }
    }
}

/// Scan configuration (merged from config file + CLI).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanConfig {
    pub path: PathBuf,
    pub exclude: Vec<String>,
    pub include: Vec<String>,
    pub min_similarity: f64,
    pub normalization: NormalizationLevel,
    pub max_clusters: Option<usize>,
    pub max_tokens: Option<usize>,
    pub jobs: Option<usize>,
    pub fail_on_error: bool,
    pub use_cache: bool,
}

impl Default for ScanConfig {
    fn default() -> Self {
        Self {
            path: PathBuf::from("."),
            exclude: Vec::new(),
            include: Vec::new(),
            min_similarity: 0.75,
            normalization: NormalizationLevel::Balanced,
            max_clusters: None,
            max_tokens: None,
            jobs: None,
            fail_on_error: false,
            use_cache: true,
        }
    }
}

// ─── Output format ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Human,
    Ai,
    Json,
    Jsonl,
}

// ─── Parse result ─────────────────────────────────────────────────────────

/// A parse error that does not terminate the scan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParseError {
    pub file: PathBuf,
    pub line: Option<usize>,
    pub column: Option<usize>,
    pub message: String,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:", self.file.display())?;
        if let Some(line) = self.line {
            write!(f, "{line}:")?;
            if let Some(col) = self.column {
                write!(f, "{col}:")?;
            }
        }
        write!(f, " {}", self.message)
    }
}

// ─── Function metadata ────────────────────────────────────────────────────

/// Classification of a function's kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FunctionKind {
    Function,
    Method,
    AsyncFunction,
    AsyncMethod,
    NestedFunction,
    Lambda,
    Process,
    Task,
    Procedure,
}

/// Extracted information about a single function, method, or HDL procedural block.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionInfo {
    #[serde(default)]
    pub language: crate::language::Language,
    pub file: PathBuf,
    pub module: String,
    pub qualified_name: String,
    pub class_name: Option<String>,
    pub function_name: String,
    pub kind: FunctionKind,
    pub start_line: usize,
    pub end_line: usize,
    pub source_bytes: usize,
    pub ast_node_count: usize,
    pub complexity: usize,
    pub decorators: Vec<String>,
    pub parameters: Vec<String>,
    pub return_annotation: Option<String>,
    pub called_functions: BTreeSet<String>,
    pub is_public: bool,
    pub is_dunder: bool,
    pub is_test: bool,
    pub is_property: bool,
    pub is_classmethod: bool,
    pub is_staticmethod: bool,
    pub is_async: bool,
    /// True when an HDL declaration was recovered after the embedded grammar
    /// could not represent valid source. Recovered units are never clustered.
    #[serde(default)]
    pub parser_recovered: bool,
}

impl FunctionInfo {
    /// Short display location: `file:name:start-end`
    pub fn short_location(&self) -> String {
        let file = self
            .file
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_else(|| self.file.display().to_string());
        format!(
            "{}:{}:{}-{}",
            file, self.function_name, self.start_line, self.end_line
        )
    }

    /// Relative location with parent dirs preserved.
    pub fn display_location(&self, base: &std::path::Path) -> String {
        let rel = self.file.strip_prefix(base).unwrap_or(&self.file);
        format!(
            "{}:{}:{}-{}",
            rel.display(),
            self.function_name,
            self.start_line,
            self.end_line
        )
    }

    /// Estimated source token count (~4 chars per token).
    pub fn estimated_tokens(&self) -> usize {
        self.source_bytes.div_ceil(4)
    }
}

// ─── Normalized representation ────────────────────────────────────────────

/// A structural token in the normalized representation.
/// Preserves semantically meaningful information while erasing superficial differences.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum StructuralToken {
    // Statements
    Assign,
    AugAssign(String), // operator: +=, -=, etc.
    Return,
    Delete,
    Assert,
    Raise,
    Pass,
    Break,
    Continue,
    Global,
    Nonlocal,
    Import(String),

    // Control flow
    If,
    Elif,
    Else,
    For,
    AsyncFor,
    While,
    Try,
    ExceptHandler,
    Finally,
    With,
    AsyncWith,
    Match,
    Case,

    // Expressions
    Call(String),      // called function/method name (preserved!)
    Attribute(String), // attribute access name (preserved!)
    BinOp(String),     // operator: +, -, *, /, etc. (preserved!)
    UnaryOp(String),   // operator: -, not, ~
    BoolOp(String),    // and, or
    Compare(String),   // ==, !=, <, >, etc.
    Subscript,
    Slice,
    Starred,

    // Comprehensions
    ListComp,
    SetComp,
    DictComp,
    GeneratorExp,

    // Literals (conditionally preserved)
    StringLiteral,
    NumericLiteral,
    BoolLiteral(bool),
    NoneLiteral,
    FStringLiteral,

    // Structural
    FunctionDef,
    AsyncFunctionDef,
    ClassDef,
    Decorator(String),
    Param,
    Lambda,
    Yield,
    YieldFrom,
    Await,

    // Name references
    ExternalName(String), // preserved for external/imported names
    LocalName,            // normalized placeholder for local names

    /// Native grammar node/keyword, preserving language-specific structure.
    Syntax(String),
    /// A scoped parameter/local identity; preserves data-flow relationships.
    LocalBinding(usize),
    /// Literal spelling retained in strict and balanced modes.
    Literal(String),

    // Block markers
    BlockStart,
    BlockEnd,
}

impl StructuralToken {
    /// Whether this token represents control flow.
    pub fn is_control_flow(&self) -> bool {
        matches!(
            self,
            Self::If
                | Self::Elif
                | Self::Else
                | Self::For
                | Self::AsyncFor
                | Self::While
                | Self::Try
                | Self::ExceptHandler
                | Self::Finally
                | Self::With
                | Self::AsyncWith
                | Self::Match
                | Self::Case
                | Self::Return
                | Self::Break
                | Self::Continue
                | Self::Raise
        )
    }
}

/// Normalized structural representation of a function.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalizedFunction {
    pub function_id: usize,
    pub tokens: Vec<StructuralToken>,
}

// ─── Fingerprints ─────────────────────────────────────────────────────────

/// Multiple deterministic fingerprints for a function.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Fingerprints {
    pub function_id: usize,
    /// SHA-256 hex of normalized token sequence serialization.
    pub exact_hash: String,
    /// 64-bit SimHash of structural token bigrams.
    pub simhash: u64,
    /// Metadata hash: param count, decorators, complexity bucket.
    pub metadata_hash: u64,
    /// Structural token type frequency vector (for Jaccard).
    pub token_frequencies: BTreeMap<String, usize>,
}

// ─── Similarity ───────────────────────────────────────────────────────────

/// Component similarity scores between two functions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimilaritySignals {
    pub ast: f64,
    pub tokens: f64,
    pub calls: f64,
    pub control_flow: f64,
    pub complexity: f64,
    pub params: f64,
}

/// Similarity result for a pair of functions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimilarityScore {
    pub func_a: usize,
    pub func_b: usize,
    pub overall: f64,
    pub signals: SimilaritySignals,
}

// ─── Structural differences ───────────────────────────────────────────────

/// Differences between structurally similar functions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructuralDiff {
    pub common_elements: Vec<String>,
    pub differences: Vec<String>,
}

// ─── Graph ────────────────────────────────────────────────────────────────

/// An edge in the similarity graph.
#[derive(Debug, Clone)]
pub struct SimilarityEdge {
    pub func_a: usize,
    pub func_b: usize,
    pub weight: f64,
}

// ─── Clustering ───────────────────────────────────────────────────────────

/// Refactoring pattern classification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RefactoringClassification {
    NearDuplicate,
    Duplicate,
    UtilityCandidate,
    HelperCandidate,
    CommonValidation,
    CommonSerialization,
    CommonErrorHandling,
    StrategyCandidate,
    AdapterCandidate,
    TemplateMethodCandidate,
    FactoryCandidate,
    RegistryCandidate,
    GenericAbstractionCandidate,
}

impl std::fmt::Display for RefactoringClassification {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::NearDuplicate => "near_duplicate",
            Self::Duplicate => "duplicate",
            Self::UtilityCandidate => "utility_candidate",
            Self::HelperCandidate => "helper_candidate",
            Self::CommonValidation => "common_validation",
            Self::CommonSerialization => "common_serialization",
            Self::CommonErrorHandling => "common_error_handling",
            Self::StrategyCandidate => "strategy_candidate",
            Self::AdapterCandidate => "adapter_candidate",
            Self::TemplateMethodCandidate => "template_method_candidate",
            Self::FactoryCandidate => "factory_candidate",
            Self::RegistryCandidate => "registry_candidate",
            Self::GenericAbstractionCandidate => "generic_abstraction_candidate",
        };
        write!(f, "{s}")
    }
}

/// A cluster of structurally similar functions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cluster {
    pub id: String,
    pub function_indices: Vec<usize>,
    pub classification: RefactoringClassification,
    pub confidence: f64,
    pub average_similarity: f64,
    pub common_structure: Vec<String>,
    pub differences: Vec<String>,
    pub duplicated_tokens_estimate: usize,
    pub potential_reduction_estimate: usize,
    pub refactoring_value: f64,
    pub signals: SimilaritySignals,
    pub reason: String,
}

// ─── Top-level scan result ────────────────────────────────────────────────

/// Statistics about the scanned repository.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepositoryStats {
    pub files_scanned: usize,
    pub files_with_errors: usize,
    pub functions_found: usize,
    pub total_source_bytes: usize,
    pub estimated_source_tokens: usize,
    pub candidate_pairs_generated: usize,
    pub clusters_found: usize,
    pub high_value_clusters: usize,
    pub scan_duration_ms: u64,
    pub cache_hits: usize,
    pub cache_misses: usize,
}

/// Complete result of a scan operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanResult {
    pub schema_version: String,
    pub tool_version: String,
    pub repository: RepositoryInfo,
    pub statistics: RepositoryStats,
    pub clusters: Vec<Cluster>,
    pub functions: Vec<FunctionInfo>,
    pub parse_errors: Vec<ParseError>,
}

/// Basic info about the scanned repository.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepositoryInfo {
    pub path: PathBuf,
    pub files: usize,
    pub functions: usize,
    pub estimated_tokens: usize,
}

// ─── Cache types ──────────────────────────────────────────────────────────

/// Cached analysis result for a single file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedFileResult {
    pub content_hash: String,
    pub tool_version: String,
    pub normalization: NormalizationLevel,
    pub functions: Vec<FunctionInfo>,
    pub normalized: Vec<NormalizedFunction>,
    pub fingerprints: Vec<Fingerprints>,
}
