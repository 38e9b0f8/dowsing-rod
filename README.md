<p align="center">
  <img src="https://raw.githubusercontent.com/38e9b0f8/dowsing-rod/main/docs/logo.svg" alt="Dowsing Rod logo" width="160">
</p>

# Dowsing Rod

Dowsing Rod finds structural refactoring opportunities in Python, JavaScript/JSX, TypeScript/TSX, C, C++, C#, Rust, Verilog, SystemVerilog, and VHDL. It is a Rust-native scanner with a standalone CLI and Python bindings, built for spotting duplicated flows, repeated procedural logic, adapter/strategy candidates, validation helpers, and other code shapes that ordinary text duplicate detectors miss.

It works offline. Source code is parsed locally, normalized locally, and rendered locally.

## Status

The current release adds native Rust scanning: functions, impl and trait methods with bodies, closures, and async fn. See [CHANGELOG.md](CHANGELOG.md) for each release. The scanner remains local-first: parsing, normalization, scoring, caching, and rendering all happen on the machine running it.

## Install

With `uv` (zero install / standalone tool):

```bash
# Run immediately without installing:
uvx dowsing-rod scan .

# Or install into your current environment:
uv pip install dowsing-rod

# Or install as a global CLI tool:
uv tool install dowsing-rod
```

With `pip`:

```bash
pip install dowsing-rod
```

With npm (no Python or compiler required):

```bash
npx dowsing-rod scan .
# or
npm install --global dowsing-rod
```

With .NET:

```bash
dotnet tool install --global dowsing-rod
```

From a local checkout:
```bash
pip install .
```

For development:

```bash
cargo test --workspace --all-targets --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
```

The project uses PyO3 and maturin for Python packaging. The npm and NuGet launchers select matching prebuilt Rust binaries. Neither has install scripts or runtime downloads.

Prebuilt command-line binaries support macOS x64/arm64, Windows x64, and glibc 2.17+ on Linux x64/arm64. That covers RHEL 8–10, current Ubuntu, Debian, Fedora, and SUSE releases. See [platform support](docs/platform-support.md) for runtime floors. musl/Alpine and Windows arm64 are not currently provided.

## Quick Start

Set up a project once, then let `auto` run from its config file. This suits hooks, CI jobs, and coding agents:

```bash
dowsing-rod init   # writes dowsing-rod.toml with the recommended defaults
dowsing-rod auto   # scans using only that file
```

`auto` takes no scan flags and refuses to run without a `dowsing-rod.toml` or a `[tool.dowsing-rod]` section in `pyproject.toml`. Like ruff and pytest, it looks in the folder it scans and then in each folder above it, so it also works from a subfolder. Edit the file to change what it does.

Scan the current project:

```bash
dowsing-rod scan .
```

Scan a specific file:

```bash
dowsing-rod scan rtl/arbiter.sv
```

Ask for compact output suitable for an LLM prompt:

```bash
dowsing-rod scan . --ai --max-tokens 4000
```

Emit structured JSON:

```bash
dowsing-rod scan . --format json
dowsing-rod scan . --format jsonl
```

## Example Human Output

```text
Dowsing Rod vX.Y.Z

Repository Summary
  Path:            /repo
  Files scanned:   42
  Functions found: 318
  Source tokens:   ~48,120
  Candidate pairs: 514
  Clusters found:  9
  High value:      3
  Scan time:       184ms

Refactoring Opportunities
  C1  Strategy candidate  funcs=3  sim=86%  duplicated=420  value=312

  C1 - Strategy candidate
    payments/stripe.py:charge:4-15
    payments/paypal.py:charge:4-15
    payments/adyen.py:charge:4-15

    Common pipeline: validate_amount -> build_charge_request -> normalize_charge_result
    Differences: call target: stripe_client vs paypal_client vs adyen_client
```

## Example AI Output

```text
DOWSING-ROD vX.Y.Z
SCHEMA 1.1
REPO /repo files=42 functions=318 tokens~48120
SCAN clusters=9 high_value=3 duration=184ms
---
C1 | strategy_candidate | signal=86% | confidence=83%
3 functions | duplicated~420 tokens | reduction~294 tokens | value=312
LANGUAGE python
MEMBERS
  payments/stripe.py:charge:4-15
  payments/paypal.py:charge:4-15
  payments/adyen.py:charge:4-15
COMMON
  validate_amount -> build_charge_request -> normalize_charge_result
DIFF
  call target: stripe_client vs paypal_client vs adyen_client
SIGNALS ast=0.88 tokens=0.84 calls=0.42 control=1.00 complexity=1.00 params=1.00
WHY
  High control-flow similarity with different call targets suggests a repeated pipeline.
---
```

## CLI Reference

```bash
dowsing-rod scan [PATH] [OPTIONS]
dowsing-rod auto [PATH]
dowsing-rod init [PATH] [--force]
dowsing-rod cache clear [PATH]
dowsing-rod version
```

Scan options:

```text
--ai                         Use compact AI-oriented output
--format human|json|jsonl    Select output format (default: config, else human)
--min-similarity FLOAT       Minimum edge similarity, default 0.75
--max-clusters N             Limit reported clusters
--max-tokens N               Token budget for AI output
--normalization LEVEL        strict, balanced, or aggressive
--jobs N                     Rayon worker thread count
--exclude PATTERN            Additional ignore pattern, repeatable
--include PATTERN            Inclusion override, repeatable
--no-cache                   Disable the file analysis cache
--fail-on-error              Return an error when any file fails to parse
--no-fail-on-error           Keep going on parse errors, even if config enables fail_on_error
```

## Python API

```python
import dowsing_rod

result = dowsing_rod.scan(
    ".",
    min_similarity=0.80,
    normalization="balanced",
    max_clusters=10,
    exclude=["generated", "vendor"],
    no_cache=True,
    fail_on_error=False,
)

for cluster in result["clusters"]:
    print(cluster["id"], cluster["classification"], cluster["average_similarity"])
```

Available functions:

```python
dowsing_rod.scan(path=".", **kwargs) -> dict
dowsing_rod.version() -> str
dowsing_rod.clear_cache(path=".") -> int
```

## Configuration

Dowsing Rod reads a standalone `dowsing-rod.toml`, or `[tool.dowsing-rod]` in `pyproject.toml`. If both exist in one folder, `dowsing-rod.toml` wins. It uses the nearest configured folder at or above the scanned path, and that folder is the project root for the cache and for displayed paths. With no configuration anywhere above, the scanned folder is the root.

`dowsing-rod init` writes this recommended starting point. It matches the built-in defaults, so it changes nothing until you edit it:

```toml
# Dowsing Rod configuration — https://github.com/38e9b0f8/dowsing-rod#configuration
# `dowsing-rod auto` runs entirely from this file. `dowsing-rod scan` also reads it,
# and its command-line flags override these values.

# Minimum similarity (0.0–1.0) for two functions to count as related.
# Raise it to report fewer, tighter clusters.
min_similarity = 0.75

# How aggressively identifiers are canonicalized: "strict", "balanced", or "aggressive".
normalization = "balanced"

# Output for `auto`, and for `scan` without --format or --ai: "human", "ai", "json", or "jsonl".
# Set "ai" or "json" for hooks, CI jobs, and coding agents.
format = "human"

# Exit with an error when any file fails to parse.
fail_on_error = false

# Extra paths or globs to skip. .gitignore, build output, virtualenvs,
# node_modules, and the cache directory are already skipped.
exclude = []

# Uncomment to limit what gets reported.
# max_clusters = 25   # most valuable clusters only
# max_tokens = 8000   # token budget for --ai / format = "ai"
```

`init` will not overwrite an existing `dowsing-rod.toml`, or shadow a `[tool.dowsing-rod]` section in `pyproject.toml`, unless you pass `--force`.

`dowsing-rod auto` uses the file alone and fails if there is none. For `scan`, any CLI flag or Python keyword argument you pass overrides the file, even when it equals the built-in default. `--ai` overrides `--format`, which overrides `format`. `--no-fail-on-error` turns off `fail_on_error = true` from the file.

Unknown keys, such as a misspelled `min_simlarity`, are ignored with a warning: on stderr for the CLI, and as a `UserWarning` in Python.

| Key | Values | Default |
|-----|--------|---------|
| `min_similarity` | `0.0`–`1.0` | `0.75` |
| `normalization` | `strict`, `balanced`, `aggressive` | `balanced` |
| `format` | `human`, `ai`, `json`, `jsonl` (CLI `auto` and `scan`) | `human` |
| `fail_on_error` | `true`, `false` | `false` |
| `max_clusters` | integer | unlimited |
| `max_tokens` | integer, used by AI output | unlimited |
| `exclude` | list of paths or globs | `[]` |
| `include` | list of globs that override excludes | `[]` |

### Example: coding agent or editor hook

`dowsing-rod auto` prints compact, budgeted output ready to paste into a prompt.

```toml
format = "ai"
max_tokens = 4000
max_clusters = 10
```

### Example: CI gate

Machine-readable output, and a parse failure fails the job.

```toml
format = "json"
fail_on_error = true
min_similarity = 0.85
exclude = ["tests/fixtures", "generated"]
```

### Example: Python project via `pyproject.toml`

```toml
[tool.dowsing-rod]
normalization = "aggressive"
exclude = ["migrations", "vendor"]
max_clusters = 25
```

### Example: only part of a monorepo

```toml
exclude = ["**/*.generated.ts", "third_party"]
include = ["services/**"]
```

## How It Works

The core pipeline is:

```text
discover supported source files
parse Python with RustPython and JavaScript, TypeScript, C, C++, C#, Rust, Verilog, SystemVerilog, and VHDL with embedded Tree-sitter grammars
extract functions, methods, lambdas, HDL tasks/functions, and HDL procedural blocks
normalize scoped structure while preserving calls, member access, operators, literals, and HDL timing/assignment syntax
fingerprint normalized tokens
generate candidate pairs with exact hashes and SimHash LSH
score candidates with multiple similarity signals
build a thresholded graph
cluster connected functions
classify and rank refactoring opportunities
render human, AI, JSON, or JSONL output
```

Normalization is deliberately semantic enough to avoid the most obvious false positives. Calls, member access, operators, control flow, literals, parameter shape, and complexity all contribute to the score. Clusters are isolated by language; JavaScript and TypeScript are separate languages.

HDL is deliberately conservative. `always` and `process` blocks are extracted but never reported as refactoring candidates. Functions, tasks, and procedures appear only for exact structural matches, as `hdl_review_required` with no reduction estimate. Treat those as a prompt to inspect clocks, resets, event/sensitivity controls, widths, assignment style, resource mapping, timing, and simulation semantics. UVM-style `.svh` headers are recovered for discovery when the embedded grammar cannot parse their macros; recovered units are never clustered.

See [docs/algorithms.md](docs/algorithms.md) for implementation details.

## Output Schema

JSON output serializes a `ScanResult` with:

```text
schema_version
tool_version
repository
statistics
clusters
functions
parse_errors
```

JSONL output writes one header object followed by one self-contained object per cluster.

## Cache

The scanner stores per-file analysis results in `.dowsing-rod-cache/`. Cache keys include file content, tool version, and normalization level. Like `.pytest_cache`, the directory contains its own `.gitignore`, so git ignores it without changes to your project's `.gitignore`.

```bash
dowsing-rod cache clear .
```

Use `--no-cache` for repeatable benchmarks or debugging.

## Benchmarks

```bash
cargo bench -p dowsing-core
```

Benchmarks cover parsing, extraction, normalization, fingerprinting, candidate generation, and synthetic pipeline scale.

## Releases

The version is set in one place: `version` under `[workspace.package]` in `Cargo.toml`. The Rust crates inherit it, maturin reads it for the Python package, MSBuild reads it for NuGet, and `scripts/package-npm.mjs` stamps it into the npm packages. Workflows read it with `node scripts/version.mjs`. To bump it:

1. Edit `version` in `Cargo.toml`.
2. Run `cargo update --workspace` to refresh `Cargo.lock` (builds use `--locked`).
3. Add a `CHANGELOG.md` entry, then tag `v<version>`.

Tool versions for CI and releases (Node, Zig, cargo-zigbuild, .NET) are likewise set once, in `.github/actions/toolchain/action.yml`.

The release workflows build and test every package before publishing. PyPI and NuGet use trusted publishing. npm uses trusted publishing after its first release; set a temporary `NPM_TOKEN` in the `npm-release` environment for that bootstrap, configure the publisher for `dowsing-rod` and the five `dowsing-rod-native-*` packages, then delete the token. Configure environment approval before the first release. Dispatch the npm and NuGet workflows from a matching `v<version>` tag with `publish` enabled.

## Comparison With Other Tools

Dowsing Rod is not a linter and does not replace formatters, type checkers, or text-level duplicate detectors.

It is most useful when:

- functions use different names but share the same flow;
- provider implementations repeat the same pipeline with different API calls;
- small validation, serialization, or helper patterns are copied across modules;
- you want a compact, ranked summary to hand to an AI coding assistant.
- you want to keep agent refactors grounded when working in a large, multi-language project.

It is less useful when:

- duplication is purely textual and already obvious;
- code cannot be parsed by its supported language frontend;
- the right abstraction depends on runtime behavior or domain constraints not present in source;
- the project is small enough that you should just do it yourself.

## Privacy

Dowsing Rod does not call external services. It reads local source files, writes an optional local cache, and emits local output. The `--ai` format is only a compact text rendering; it does not send code to any model.

## License

MIT. See [LICENSE](LICENSE) and [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
