# Dowsing Rod

Dowsing Rod finds structural refactoring opportunities in Python codebases. It is a Rust-native scanner with Python bindings and a CLI, built for spotting duplicated flows, adapter/strategy candidates, validation helpers, serialization patterns, and other code shapes that ordinary text duplicate detectors miss.

It works offline. Source code is parsed locally, normalized locally, and rendered locally.

## Status

`0.1.0` is an early implementation. The core pipeline, renderers, Python bindings, CLI entrypoint, fixtures, benchmarks, and CI workflow are present. Expect tuning as the similarity model is tested on larger real repositories.

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

From a local checkout:
```bash
pip install .
```

For development:

```bash
cargo test --all-targets
cargo clippy --all-targets --all-features -- -D warnings
```

The project uses PyO3 and maturin for Python packaging. CI builds wheels with the stable Rust toolchain.

## Quick Start

Scan the current project:

```bash
dowsing-rod scan .
```

Scan a specific file:

```bash
dowsing-rod scan app/services/payments.py
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
Dowsing Rod v0.1.0

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
DOWSING-ROD v0.1.0
SCHEMA 1.0
REPO /repo files=42 functions=318 tokens~48120
SCAN clusters=9 high_value=3 duration=184ms
---
C1 | strategy_candidate | signal=86% | confidence=83%
3 functions | duplicated~420 tokens | reduction~294 tokens | value=312
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
dowsing-rod cache clear [PATH]
dowsing-rod version
```

Scan options:

```text
--ai                         Use compact AI-oriented output
--format human|json|jsonl    Select output format
--min-similarity FLOAT       Minimum edge similarity, default 0.75
--max-clusters N             Limit reported clusters
--max-tokens N               Token budget for AI output
--normalization LEVEL        strict, balanced, or aggressive
--jobs N                     Rayon worker thread count
--exclude PATTERN            Additional ignore pattern, repeatable
--include PATTERN            Inclusion override, repeatable
--no-cache                   Disable the file analysis cache
--fail-on-error              Return an error when any file fails to parse
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

Dowsing Rod reads `[tool.dowsing-rod]` from `pyproject.toml` in the project root.

```toml
[tool.dowsing-rod]
min_similarity = 0.78
normalization = "balanced"
max_clusters = 25
max_tokens = 8000
exclude = ["generated", "vendor", "migrations"]
include = ["src"]
```

CLI flags and Python keyword arguments override file configuration where an override is provided.

## How It Works

The core pipeline is:

```text
discover Python files
parse with RustPython
extract function metadata
normalize AST structure
fingerprint normalized tokens
generate candidate pairs with exact hashes and SimHash LSH
score candidates with multiple similarity signals
build a thresholded graph
cluster connected functions
classify and rank refactoring opportunities
render human, AI, JSON, or JSONL output
```

Normalization is deliberately semantic enough to avoid the most obvious false positives. Calls, attributes, operators, control flow, decorators, async-ness, parameter shape, and complexity all contribute to the score.

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

The scanner stores per-file analysis results in `.dowsing-rod-cache/`. Cache keys include file content, tool version, and normalization level.

```bash
dowsing-rod cache clear .
```

Use `--no-cache` for repeatable benchmarks or debugging.

## Benchmarks

```bash
cargo bench -p dowsing-core
```

Benchmarks cover parsing, extraction, normalization, fingerprinting, candidate generation, and synthetic pipeline scale.

## Comparison With Other Tools

Dowsing Rod is not a linter and does not replace formatters, type checkers, or text-level duplicate detectors.

It is most useful when:

- functions use different names but share the same flow;
- provider implementations repeat the same pipeline with different API calls;
- small validation, serialization, or helper patterns are copied across modules;
- you want a compact, ranked summary to hand to an AI coding assistant.

It is less useful when:

- duplication is purely textual and already obvious;
- code cannot be parsed as Python;
- the right abstraction depends on runtime behavior or domain constraints not present in source.

## Privacy

Dowsing Rod does not call external services. It reads local source files, writes an optional local cache, and emits local output. The `--ai` format is only a compact text rendering; it does not send code to any model.

## License

MIT. See [LICENSE](LICENSE).
