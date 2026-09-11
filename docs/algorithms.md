# Dowsing Rod Algorithms

This document describes the current `0.1.0` analysis pipeline. The implementation is intentionally deterministic and local-first: every phase runs in-process over source files and produces serializable evidence for renderers and downstream tools.

## Pipeline

```text
config
  -> discovery
  -> parsing
  -> extraction
  -> normalization
  -> fingerprinting
  -> candidate generation
  -> similarity scoring
  -> graph construction
  -> clustering
  -> classification
  -> ranking
  -> rendering
```

## Configuration

`ScanConfig` starts from defaults, then merges `[tool.dowsing-rod]` from `pyproject.toml`, then applies caller-provided overrides. The scan target can be a directory or a single `.py` file.

The project root is used for config lookup, cache storage, and relative display paths. For a single-file scan, the project root is the file's parent directory.

## Discovery

Discovery uses the `ignore` crate for recursive directory scans. It respects project `.gitignore` files and applies default exclusions for common generated/cache environments:

```text
.git, .venv, venv, env, __pycache__, .pytest_cache, .mypy_cache,
.ruff_cache, .tox, .nox, dist, build, node_modules, site-packages,
.eggs, *.egg-info, .dowsing-rod-cache
```

User exclusions are added as override globs. User inclusions are added after exclusions so they can opt paths back in.

Single-file scans bypass the directory walker and return the file directly when it has a `.py` extension.

## Parsing

Parsing uses `rustpython-parser` in module mode. Parse failures are converted into `ParseError` values with file, row, column, and parser message. The scanner normally keeps going when a file fails to parse. With `fail_on_error = true`, the core API returns an error after collecting parse errors.

## Extraction

Extraction walks the Python AST and records every top-level function, method, async function, async method, and nested function it visits.

For each function, Dowsing Rod records:

- file, module, qualified name, class name, and source line range;
- function kind and flags such as async, public, dunder, test, property, classmethod, staticmethod;
- parameters and return annotation;
- called function and method names;
- approximate source byte count;
- AST node count;
- cyclomatic complexity estimate.

Complexity starts at 1 and increments for decision points such as `if`, `for`, `while`, `except`, `assert`, boolean operators, and conditional expressions.

## Normalization

Normalization converts a function body into a sequence of `StructuralToken` values. These tokens keep important semantics while removing superficial differences.

Always preserved:

- control flow shape;
- operators;
- called function and method names;
- attribute names;
- decorators;
- boolean and `None` literals;
- async boundaries such as `await`;
- block boundaries.

Normalization levels:

```text
strict      preserve all names as external names
balanced    normalize parameters and local variables; preserve calls and attributes
aggressive  like balanced, with literals treated generically
```

Balanced normalization is the default because it catches renamed-variable clones without erasing call targets such as `db.save` versus `db.delete`.

## Fingerprinting

Each normalized function gets multiple fingerprints:

- `exact_hash`: deterministic SipHash of the normalized token sequence;
- `simhash`: 64-bit SimHash over token bigrams;
- `metadata_hash`: hash of parameter count, complexity bucket, async flag, and decorators;
- `token_frequencies`: multiset counts of structural token categories.

Exact hashes catch direct structural duplicates. SimHash supports approximate lookup by Hamming distance.

## Candidate Generation

Candidate generation is intentionally cheaper than full similarity scoring.

It combines:

- exact-hash groups, where every function in a group becomes a candidate pair;
- SimHash locality-sensitive buckets;
- Hamming-distance checks inside buckets.

This keeps the expensive scoring phase focused on pairs with plausible structural overlap.

## Similarity Scoring

Each candidate pair receives a weighted score from six signals:

```text
AST structure    0.30  normalized-token LCS ratio
token similarity 0.25  multiset Jaccard over structural tokens
call similarity  0.20  Jaccard over called function names
control flow     0.15  LCS ratio over control-flow tokens only
complexity       0.05  smaller/larger cyclomatic complexity ratio
parameters       0.05  parameter count plus async compatibility
```

The call signal is deliberately strong enough to suppress false positives where two functions have the same shape but opposite semantics.

## Graph And Clustering

The similarity graph has one node per function. A candidate pair becomes an edge when its overall similarity is at or above `min_similarity`.

Clusters are connected components in this graph. Oversized components are split by removing the weakest edges until each resulting component is at or below the maximum cluster size.

Cluster evidence includes:

- member function indices;
- average pairwise similarity;
- averaged signal scores;
- common call pipeline;
- representative structural differences;
- duplicated-token and reduction estimates.

## Classification

Classification is heuristic and conservative. It labels a cluster as a refactoring candidate, not as a mandatory refactor.

Current labels include:

- `near_duplicate`;
- `duplicate`;
- `utility_candidate`;
- `helper_candidate`;
- `common_validation`;
- `common_serialization`;
- `common_error_handling`;
- `strategy_candidate`;
- `adapter_candidate`;
- `template_method_candidate`;
- `factory_candidate`;
- `registry_candidate`;
- `generic_abstraction_candidate`.

Examples:

- Very high AST and token similarity becomes a near duplicate or duplicate.
- High control-flow similarity with lower call similarity across 3+ members becomes a strategy candidate.
- Similar wrappers over different external APIs become adapter candidates.
- Small repeated guard functions become common validation candidates.

## Ranking

Clusters are ranked by estimated refactoring value:

```text
duplicated_tokens * average_similarity * confidence * log2(cluster_size)
```

This means a larger cluster with slightly lower similarity can rank above a tiny exact duplicate when it offers more practical reduction.

After ranking, cluster IDs are reassigned deterministically as `C1`, `C2`, and so on.

## Rendering

Renderers are separate from analysis:

- human output prints a readable terminal report;
- AI output prints compact evidence lines and respects `max_tokens`;
- JSON output serializes the full `ScanResult`;
- JSONL output writes a header line followed by one self-contained cluster line per cluster.

Token estimates use a rough 4-characters-per-token heuristic. The scanner does not call a model or tokenizer.

## Known Limitations

- Similarity is static and source-based; runtime behavior and domain invariants are outside the model.
- Literal values are not yet richly compared beyond token categories.
- Common pipeline extraction is call-name based and can miss equivalent calls with very different naming.
- Candidate generation can miss pairs when SimHash buckets diverge despite meaningful semantic similarity.
- Python syntax support follows the bundled RustPython parser version.
