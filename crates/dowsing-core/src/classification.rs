use crate::types::{FunctionInfo, RefactoringClassification, SimilaritySignals};

/// Classify a cluster into a refactoring pattern using heuristics.
///
/// Returns (classification, confidence, reason).
/// Never asserts that a refactoring IS correct — only that it's a candidate with confidence.
pub fn classify_cluster(
    members: &[&FunctionInfo],
    signals: &SimilaritySignals,
    avg_similarity: f64,
) -> (RefactoringClassification, f64, String) {
    if members.is_empty() {
        return (
            RefactoringClassification::GenericAbstractionCandidate,
            0.5,
            "insufficient data".to_string(),
        );
    }

    // --- Near/exact duplicate ---
    if avg_similarity >= 0.97 && signals.ast >= 0.97 {
        return (
            RefactoringClassification::NearDuplicate,
            avg_similarity,
            "Near-identical AST structure across all members.".to_string(),
        );
    }
    if avg_similarity >= 0.92 && signals.tokens >= 0.92 {
        return (
            RefactoringClassification::Duplicate,
            avg_similarity,
            "Very high token and AST similarity — likely copy-pasted code.".to_string(),
        );
    }

    if members.iter().any(|member| member.language.is_hdl()) {
        return (RefactoringClassification::GenericAbstractionCandidate, avg_similarity.min(0.85),
            "Repeated HDL structure. Review timing, widths, reset behavior, and synthesis semantics before sharing logic.".into());
    }

    // --- Strategy candidate ---
    // High control flow similarity, low call similarity (different implementations)
    let is_strategy = signals.control_flow >= 0.80
        && signals.calls < 0.60
        && avg_similarity >= 0.75
        && members.len() >= 3;
    if is_strategy {
        let confidence = (signals.control_flow * 0.5 + avg_similarity * 0.5).min(0.99);
        return (
            RefactoringClassification::StrategyCandidate,
            confidence,
            format!(
                "High control-flow similarity ({:.0}%) with different call targets suggests \
                 a repeated pipeline with provider-specific implementations.",
                signals.control_flow * 100.0
            ),
        );
    }

    // --- Adapter candidate ---
    // Different call sets, similar AST, functions likely wrapping different external APIs
    let is_adapter = signals.ast >= 0.78
        && signals.calls < 0.50
        && members.iter().all(|f| f.parameters.len() <= 4)
        && members.len() >= 2;
    if is_adapter {
        let confidence = (signals.ast * 0.5 + avg_similarity * 0.5).min(0.95);
        return (
            RefactoringClassification::AdapterCandidate,
            confidence,
            "Similar structure wrapping different external APIs — possible Adapter abstraction."
                .to_string(),
        );
    }

    // --- Common validation ---
    // Short functions with assert/if-raise patterns
    let is_validation = members
        .iter()
        .all(|f| f.ast_node_count <= 15 && f.complexity <= 4)
        && signals.control_flow >= 0.75
        && avg_similarity >= 0.78;
    if is_validation {
        let confidence = (avg_similarity * 0.7 + signals.control_flow * 0.3).min(0.95);
        return (
            RefactoringClassification::CommonValidation,
            confidence,
            "Small functions with similar validation/guard patterns.".to_string(),
        );
    }

    // --- Common serialization ---
    let serialization_calls = [
        "serialize",
        "deserialize",
        "to_dict",
        "to_json",
        "from_dict",
        "from_json",
        "encode",
        "decode",
        "marshal",
        "unmarshal",
        "dump",
        "load",
        "dumps",
        "loads",
    ];
    let is_serialization = members.iter().any(|f| {
        f.called_functions
            .iter()
            .any(|c| serialization_calls.iter().any(|s| c.contains(s)))
    }) && avg_similarity >= 0.78;
    if is_serialization {
        let confidence = avg_similarity.min(0.92);
        return (
            RefactoringClassification::CommonSerialization,
            confidence,
            "Similar serialization/deserialization patterns.".to_string(),
        );
    }

    // --- Common error handling ---
    let error_calls = [
        "raise",
        "except",
        "log",
        "logger",
        "error",
        "exception",
        "handle",
    ];
    let is_error_handling = signals.control_flow >= 0.80
        && members.iter().all(|f| {
            f.called_functions
                .iter()
                .any(|c| error_calls.iter().any(|s| c.contains(s)))
        })
        && avg_similarity >= 0.75;
    if is_error_handling {
        let confidence = (signals.control_flow * 0.6 + avg_similarity * 0.4).min(0.93);
        return (
            RefactoringClassification::CommonErrorHandling,
            confidence,
            "Similar error handling control flow patterns.".to_string(),
        );
    }

    // --- Factory candidate ---
    let construction_calls = [
        "new",
        "create",
        "build",
        "make",
        "construct",
        "__init__",
        "from_",
    ];
    let is_factory = members.iter().all(|f| {
        f.called_functions.iter().any(|c| {
            construction_calls
                .iter()
                .any(|s| c.starts_with(s) || c.contains(s))
        })
    }) && avg_similarity >= 0.76;
    if is_factory {
        let confidence = avg_similarity.min(0.90);
        return (
            RefactoringClassification::FactoryCandidate,
            confidence,
            "Similar object construction patterns — possible Factory abstraction.".to_string(),
        );
    }

    // --- Utility candidate ---
    // Small, similar functions not fitting other patterns
    let is_utility = members.iter().all(|f| f.ast_node_count <= 20)
        && avg_similarity >= 0.80
        && members.len() >= 2;
    if is_utility {
        let confidence = avg_similarity.min(0.90);
        return (
            RefactoringClassification::UtilityCandidate,
            confidence,
            "Small functions with similar structure — possible shared utility extraction."
                .to_string(),
        );
    }

    // --- Helper candidate ---
    let is_helper = avg_similarity >= 0.78 && members.len() >= 2;
    if is_helper {
        let confidence = avg_similarity.min(0.88);
        return (
            RefactoringClassification::HelperCandidate,
            confidence,
            "Structurally similar helper functions with shared patterns.".to_string(),
        );
    }

    // --- Template method candidate ---
    // High overall similarity, mixed call similarity
    let is_template = avg_similarity >= 0.75 && signals.control_flow >= 0.70 && members.len() >= 2;
    if is_template {
        let confidence = (avg_similarity * 0.6 + signals.control_flow * 0.4).min(0.87);
        return (
            RefactoringClassification::TemplateMethodCandidate,
            confidence,
            "Similar structure with overridable steps — possible Template Method.".to_string(),
        );
    }

    // Default
    (
        RefactoringClassification::GenericAbstractionCandidate,
        avg_similarity * 0.8,
        format!(
            "Structural similarity ({:.0}%) suggests potential for shared abstraction.",
            avg_similarity * 100.0
        ),
    )
}

/// Return a human-readable label for a classification.
pub fn classification_label(c: &RefactoringClassification) -> &'static str {
    match c {
        RefactoringClassification::NearDuplicate => "Near duplicate",
        RefactoringClassification::Duplicate => "Duplicate",
        RefactoringClassification::UtilityCandidate => "Utility candidate",
        RefactoringClassification::HelperCandidate => "Helper candidate",
        RefactoringClassification::CommonValidation => "Validation candidate",
        RefactoringClassification::CommonSerialization => "Serialization candidate",
        RefactoringClassification::CommonErrorHandling => "Error handling candidate",
        RefactoringClassification::StrategyCandidate => "Strategy candidate",
        RefactoringClassification::AdapterCandidate => "Adapter candidate",
        RefactoringClassification::TemplateMethodCandidate => "Template method candidate",
        RefactoringClassification::FactoryCandidate => "Factory candidate",
        RefactoringClassification::RegistryCandidate => "Registry candidate",
        RefactoringClassification::GenericAbstractionCandidate => "Abstraction candidate",
    }
}
