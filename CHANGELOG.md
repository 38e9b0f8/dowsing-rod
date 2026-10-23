# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.3.0] - Unreleased

### Added

- Native structural analysis for Rust: functions, impl and trait methods with bodies, nested functions, closures, and async fn.
- Normalization of Rust control flow (`if`/`match`/`loop`/`while`/`for`/`?`), `self` vs locals, destructured bindings, path call names, and macro invocations.
- `dowsing-rod init` writes a `dowsing-rod.toml` with the recommended default configuration.
- `dowsing-rod auto` scans using only the project's configuration file, for hooks, CI, and coding agents. It fails when no configuration exists.
- `format` and `fail_on_error` configuration keys. `scan` uses `format` when neither `--format` nor `--ai` is passed.
- The cache directory writes its own `.gitignore`, so git ignores it automatically.
- `--no-fail-on-error` turns off `fail_on_error = true` from configuration.
- Unknown configuration keys produce a warning instead of being silently ignored.

### Changed

- Configuration is found in the scanned folder or the nearest folder above it, which also becomes the project root for the cache and displayed paths.
- `dowsing_core::scan` no longer reads configuration files. Callers build a `ScanConfig` from `config::load_file_config` and apply their own options on top.
- The version is set once, in `Cargo.toml`. `pyproject.toml`, the npm packages, and the NuGet package derive it, and CI tool versions live in `.github/actions/toolchain`.

### Fixed

- `max_tokens` from configuration now applies to CLI AI output.
- CLI flags and Python keyword arguments now override configuration even when they equal the default, for example `--min-similarity 0.75` over a configured `0.9`.
- `scripts/test-npm.mjs` accepts npm 12's `npm pack --json` output.

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
