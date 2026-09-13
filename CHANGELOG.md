# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.0] - Unreleased

### Added

- Native structural analysis for JavaScript/JSX, TypeScript/TSX, C, C++, C#, Verilog, SystemVerilog, and VHDL.
- Extraction of functions, methods, lambdas, HDL functions/tasks/procedures, and HDL `always`/`process` blocks.
- Language-isolated candidate generation and clustering, including separate JavaScript and TypeScript findings.
- A standalone Rust CLI with npm and NuGet launcher packages backed by prebuilt platform binaries.
- Offline packed-install validation and review-gated npm, NuGet, and PyPI release workflows.
- C/C++ selection for ambiguous `.h` headers based on parser error regions.

### Changed

- Candidate generation now uses bounded per-language sets and report-sized exact-duplicate groups to avoid quadratic scoring on large repositories.
- AI output identifies the language for every reported cluster.

## [0.1.0] - Unreleased

### Added

- Initial release
- Rust-native Python structural analysis engine
- Function extraction with full metadata
- AST normalization (strict, balanced, aggressive)
- SimHash-based locality-sensitive candidate generation
- Multi-signal detailed similarity scoring
- Structural difference extraction
- Graph-based clustering with connected components
- Heuristic refactoring pattern classification
- Refactoring value estimation and ranking
- Approximate token economics
- File-content-based caching
- Parallel analysis via rayon
- CLI: `scan`, `cache`, `version`
- `--ai` output mode for AI coding agents
- JSON and JSONL output formats
- Python API: `scan()`, `version()`, `clear_cache()`
- Prebuilt wheels for Linux, macOS, Windows
