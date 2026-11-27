# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.3.0] - Unreleased

### Added

- Native structural analysis for Java (`.java`): methods, constructors, record compact constructors, lambdas, and methods of anonymous and local classes. Modern syntax is understood: switch expressions, pattern matching, text blocks, try-with-resources, and records. Trivial getters and setters are skipped, and annotations and modifiers are metadata, not structure.
- Maven distribution, published to Maven Central under `io.github.shkb42`. `dowsing-rod-maven-plugin` adds the `scan`, `auto`, `init`, `check` (build gate), `cache-clear`, and `help` goals and runs once per reactor by default. `dowsing-rod-java` is a dependency-free Java 8+ API. `dowsing-rod-native` holds the per-platform binaries, verified by SHA-256. Java 8+ and Maven 3.6.3+ are enough; no Python, Node, or .NET is needed.
- Static musl builds for Linux x64 and arm64 (Alpine). They ship as the npm packages `dowsing-rod-native-linux-{x64,arm64}-musl`, the NuGet RIDs `linux-musl-{x64,arm64}`, and the Maven classifiers `linux-{x86_64,aarch_64}-musl`. The npm and NuGet launchers pick them automatically instead of refusing to run on musl.
- Native structural analysis for Rust: functions, impl and trait methods with bodies, nested functions, closures, and async fn.
- Normalization of Rust control flow (`if`/`match`/`loop`/`while`/`for`/`?`), `self` vs locals, destructured bindings, path call names, and macro invocations.
- `dowsing-rod init` writes a `dowsing-rod.toml` with the recommended default configuration.
- `dowsing-rod auto` scans using only the project's configuration file, for hooks, CI, and coding agents. It fails when no configuration exists.
- `format` and `fail_on_error` configuration keys. `scan` uses `format` when neither `--format` nor `--ai` is passed.
- The cache directory writes its own `.gitignore`, so git ignores it automatically.
- Security and supply-chain CI: `security.yml` (CodeQL, zizmor and actionlint, gitleaks, cargo-deny, dependency review), OpenSSF Scorecard, and Dependabot. Every action is pinned to a commit SHA, release jobs use no caches, and Maven releases add CycloneDX SBOMs, build-provenance attestations, and a check that published jars are byte-identical to the tested ones.
- `--no-fail-on-error` turns off `fail_on_error = true` from configuration.
- Unknown configuration keys produce a warning instead of being silently ignored.

### Changed

- `.java` files are now scanned. On mixed-language repositories that contain Java, a scan reports more functions and clusters than before. To keep the previous results, add `exclude = ["**/*.java"]`. Output for every other language is unchanged.
- `.gradle` is skipped by default, like `target` and `build`.
- Configuration is found in the scanned folder or the nearest folder above it, which also becomes the project root for the cache and displayed paths.
- `dowsing_core::scan` no longer reads configuration files. Callers build a `ScanConfig` from `config::load_file_config` and apply their own options on top.
- The version is set once, in `Cargo.toml`. `pyproject.toml`, the npm packages, and the NuGet package derive it, and CI tool versions live in `.github/actions/toolchain`.

### Fixed

- `max_tokens` from configuration now applies to CLI AI output.
- CLI flags and Python keyword arguments now override configuration even when they equal the default, for example `--min-similarity 0.75` over a configured `0.9`.
- `scripts/test-npm.mjs` accepts npm 12's `npm pack --json` output.
- The NuGet release workflow keeps each native binary in its `<rid>/` folder when it passes them between jobs; it previously failed at the "Check staged native binaries" step.

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
