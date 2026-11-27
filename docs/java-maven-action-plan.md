# Action Plan: Java Language Support and Maven Distribution

Status: **Phases 0–6 implemented on branch `feature/java-maven`** (2026-11-15). Decisions taken:

- **D2 groupId:** `io.github.shkb42` (this fork publishes the Maven channel; upstream links stay on `38e9b0f8`).
- **D3 Java floor:** Java **8**.
- **D4 Maven:** Maven 3.6.3+.
- **D5 musl:** included, for npm, NuGet, and Maven.
- **D6 snapshots:** none.
- **D7 schema:** `schema_version` stays `1.1`, so strict consumers are unaffected.
- **D8 gate:** the `check` thresholds are off by default.
- **D9 Gradle:** stays a follow-up.

Still to do before the first Central release: the Maven Central namespace, the GPG key, and the release secrets. The step-by-step checklist is in [TODO.md](../TODO.md). Phase 7 items remain optional. The sections below are the original plan.

Date: 2026-11-15 · Baseline: `main` at `21cf8a8` (version `0.3.0`)

---

## 1. Summary

The goal is for Java teams to use Dowsing Rod the way Java teams use any other tool: add it to `pom.xml`, run `mvn verify`, get findings, and fail the build when wanted. They should need only a JDK and Maven, with no Python, Node, or .NET. Everything Dowsing Rod does today must keep working: offline analysis, structural clustering, AI output, config files, caching, and the CI gate.

That breaks into two separate workstreams:

| | Workstream | What it delivers |
|---|---|---|
| **A** | **Java language frontend** in the Rust core | Dowsing Rod can *understand Java source* (methods, constructors, lambdas, records, etc.). |
| **B** | **Maven distribution** | Dowsing Rod can be *installed and run from Maven*, with versions published to Maven Central from the single `Cargo.toml` version. |

**Key finding:** the scanner does not depend on Python. The engine is a self-contained Rust binary (`crates/dowsing-cli`). Python is only one distribution channel (PyPI wheel via PyO3/maturin), next to npm and NuGet, which already ship the same binary without Python. So **no rewrite is needed**. The recommended design copies the proven npm/NuGet pattern: Maven artifacts carry the prebuilt native binary for each platform, and a thin Java layer (Maven plugin + small Java API) runs it.

**Recommendation:** keep one Rust engine, add a tree-sitter Java frontend, and publish a `dowsing-rod-maven-plugin` that resolves a per-platform native artifact from Maven Central. Do not port the engine to Java.

---

## 2. What the project is today

### 2.1 Purpose

Dowsing Rod finds **structural refactoring opportunities** that text diff tools miss. Examples are functions with different names that share one control-flow pipeline (strategy/adapter candidates), template-method skeletons, repeated validation and serialization helpers, near-duplicates, and test duplication that could become parametrized tests. It ranks the findings and prints them for people (`human`), coding agents (`--ai`, token-budgeted), or machines (`json`/`jsonl`). It runs fully offline.

### 2.2 Architecture

```text
crates/
  dowsing-core     Rust engine: discovery → parse → extract → normalize → fingerprint
                   → candidates → similarity → graph → cluster → classify → rank → render
  dowsing-cli      `dowsing-rod` binary (scan / auto / init / cache clear / version)
  dowsing-python   PyO3 bindings (scan(), version(), clear_cache(), CLI entry point)

Distribution (all ship the SAME native binary from dowsing-cli):
  PyPI   pyproject.toml + maturin         → wheel with native module + CLI
  npm    npm/ + scripts/package-npm.mjs   → launcher + 5 dowsing-rod-native-<platform> packages
  NuGet  nuget/ + scripts/package-nuget.mjs → dotnet tool with native/<rid>/ binaries
```

Main files for language support:

| File | Role |
|---|---|
| [crates/dowsing-core/src/language.rs](../crates/dowsing-core/src/language.rs) | `Language` enum, extension mapping, grammar selection |
| [crates/dowsing-core/src/native.rs](../crates/dowsing-core/src/native.rs) | Generic tree-sitter frontend: unit detection, names, parameters, local scopes, normalization, calls, literals, control-flow tokens |
| [crates/dowsing-core/src/lib.rs](../crates/dowsing-core/src/lib.rs) | Pipeline; dispatches Python → RustPython, everything else → `native::analyze` |
| [crates/dowsing-core/src/classification.rs](../crates/dowsing-core/src/classification.rs) | Pattern classification (with a special HDL path) |
| [crates/dowsing-core/src/discovery.rs](../crates/dowsing-core/src/discovery.rs) | File walking and default excludes (`target`, `build`, … already excluded) |
| [crates/dowsing-core/src/config.rs](../crates/dowsing-core/src/config.rs) | `dowsing-rod.toml` / `[tool.dowsing-rod]` lookup, `init`, validation |

Main files for distribution and release:

| File | Role |
|---|---|
| `Cargo.toml` `[workspace.package].version` | **The only place the version is set** |
| [scripts/version.mjs](../scripts/version.mjs) | Prints the version; `--check-tag` makes sure the release tag matches |
| [.github/workflows/build-native.yml](../.github/workflows/build-native.yml) | Reusable matrix: builds and tests 5 platform binaries (glibc 2.17 floor, macOS 10.12/11, static CRT on Windows) |
| [.github/workflows/npm-release.yml](../.github/workflows/npm-release.yml), [nuget-release.yml](../.github/workflows/nuget-release.yml) | Review-gated publishing that reuses `build-native.yml` |
| [.github/actions/toolchain/action.yml](../.github/actions/toolchain/action.yml) | All CI tool versions, pinned in one place |
| [nuget/dowsing-rod/Program.cs](../nuget/dowsing-rod/Program.cs) | Launcher pattern: detect platform → pick bundled binary → exec → forward exit code |

### 2.3 Where Python is actually involved

| Touchpoint | Effect on a Java user | Action |
|---|---|---|
| PyPI wheel / `uvx` / `pip` install paths | Not needed. They are just one channel. | None. Maven becomes another channel. |
| `[tool.dowsing-rod]` in `pyproject.toml` | Java projects don't have one. `dowsing-rod.toml` already works on its own. | None for the core. The Maven plugin adds `pom.xml` configuration (§5.4). |
| CI `test` job installs Python for smoke tests | Internal only | The new Java CI job does not use Python. |
| Docs/`llms.txt` recommend `uvx`/`npx` | Java users see no Maven path | Add Maven instructions. |

The Node scripts (`scripts/*.mjs`) only run in CI and at build time. End users never need Node.

---

## 3. Goals and non-goals

### Goals

1. **Java source analysis** with the same quality bar as the other languages: extraction, normalization, clustering, classification, all output formats.
2. **Native Maven experience**:
   - `mvn dowsing-rod:scan`, `dowsing-rod:auto`, `dowsing-rod:init`, `dowsing-rod:check`;
   - configuration in `pom.xml` and/or `dowsing-rod.toml`;
   - works in multi-module reactors;
   - fails the build on demand.
3. **Maven Central publishing** with the version taken from `Cargo.toml`, a review-gated release, signed artifacts, and checks that the tag matches the version, like the npm and NuGet flows.
4. **No Python, Node, or .NET for end users.** A JDK and Maven are enough. Everything resolves through normal Maven repositories, so it works behind Nexus/Artifactory mirrors and in air-gapped builds.
5. **No regressions** for existing users, languages, channels, or the JSON schema.

### Non-goals (for this plan; some are listed as later phases)

- Porting the analysis engine to Java.
- Kotlin, Scala, or Groovy analysis. They could follow later on the same frontend pattern.
- Gradle plugin in v1. It is designed for here and scheduled as an optional Phase 7.
- Semantic or type-resolved analysis, such as compiling the project or resolving classpaths. The tool stays syntax-based and offline, like every other frontend.

---

## 4. Workstream A — Java language frontend (Rust core)

### 4.1 Approach

Add the `tree-sitter-java` grammar. It is MIT-licensed and is published on crates.io against the same `tree-sitter-language` ABI as the grammars already used. Extend the existing generic `native.rs` frontend instead of writing a new one. This follows exactly how C# and Rust were added, and the Rust commit (`3383338`) is the file-by-file template.

### 4.2 Language registration

- `language.rs`:
  - add `Language::Java`;
  - map `.java` to it;
  - wire up `grammar()` → `tree_sitter_java::LANGUAGE`;
  - `Display` → `"java"`. Serde gets `"java"` from `snake_case`.
- `Cargo.toml` / `crates/dowsing-core/Cargo.toml`: add `tree-sitter-java` (version pinned and lock-checked in Phase 0).
- `THIRD_PARTY_NOTICES.md`: add the grammar's MIT notice.

### 4.3 Grammar mapping (tree-sitter-java node kinds → existing concepts)

| Concept in `native.rs` | Java node kinds to add |
|---|---|
| **Units** (`unit_kind`) | `method_declaration` *with a body* → `Method` (abstract and interface methods without bodies are skipped, like C#) · `constructor_declaration` → `Method` · `compact_constructor_declaration` (records) → `Method` · `lambda_expression` → `Lambda` · methods inside anonymous classes and local classes → `Method`/`NestedFunction` · `static_initializer`: decide in Phase 1 (proposal: extract but don't report, as with HDL processes, or skip entirely) |
| **Scopes / class name** | `class_declaration`, `interface_declaration`, `enum_declaration`, `record_declaration`, `annotation_type_declaration`; anonymous `object_creation_expression` bodies (`<anonymous Type>`) |
| **Parameters** | `formal_parameters` → `formal_parameter` (`name`), `spread_parameter` (varargs), skip `receiver_parameter` (like Rust's `self_parameter`); lambda `identifier`, `inferred_parameters` |
| **Local bindings** (`register`) | `local_variable_declaration` → `variable_declarator` (already handled) · `enhanced_for_statement` `name` · `catch_formal_parameter` · try-with-resources `resource` · pattern variables (`instanceof` patterns, `type_pattern`, `record_pattern` in `switch`) |
| **Lexical scopes** | `block`, `constructor_body`, `for_statement`, `enhanced_for_statement`, `catch_clause`, `switch_block_statement_group`, `switch_rule`, `lambda_expression` |
| **Control-flow tokens** | `if_statement`, `ternary_expression` → If · `for_statement`, `enhanced_for_statement` → For · `while_statement`, `do_statement` → While · `switch_expression` (covers statement + expression forms) → Match · `switch_block_statement_group`, `switch_rule` → Case · `return`/`break`/`continue`/`yield` · `try_statement`, `try_with_resources_statement` → Try · `throw_statement` → Raise · `catch_clause` → ExceptHandler · `synchronized_statement`, `assert_statement` (complexity) |
| **Calls** (`call_target`) | `method_invocation` (`object` + `name`), `object_creation_expression` (`new Type`, important for spotting provider clients), `explicit_constructor_invocation` (`this(...)`/`super(...)`), `method_reference` (`Type::method`) |
| **Member access** (`is_member`) | `field_access` (`object`.`field`), the `name` of `method_invocation` when an `object` is present. This keeps `this.stripeClient` vs `this.paypalClient` as meaningful differences. |
| **Literals** | `decimal_integer_literal`, `hex_integer_literal` (already present), plus `octal_integer_literal`, `binary_integer_literal`, `decimal_floating_point_literal`, `hex_floating_point_literal`, `text_block`, `null_literal`, `true`/`false` |
| **Comments** | `line_comment`, `block_comment`. Already skipped by the generic `contains("comment")` rule, and Javadoc is a `block_comment`. |

**Call-target spelling:** for `a.b(x).c(y)`, the target is written `a.b(..).c`: the receiver chain stays, the argument text is dropped. The call signal then compares `client.charge` with `client.refund` correctly, without argument noise. This matches how the TypeScript frontend treats member calls.

### 4.4 Java-specific metadata (`FunctionInfo`)

Today the native frontends hard-code some Python-flavoured heuristics. Java needs its own branches:

| Field | Current native behaviour | Java behaviour |
|---|---|---|
| `decorators` | always empty | Annotation names from `modifiers` (`Override`, `Test`, `ParameterizedTest`, `Transactional`, …). **Drop pure noise annotations** (`Override`, `SuppressWarnings`, `Nullable`/`NonNull`, `Deprecated`): `decorators` feeds `metadata_hash`, so noise would hurt candidate matching. |
| `is_public` | `!name.starts_with('_')` (always true in Java) | `public` modifier, or an interface member |
| `is_staticmethod` | false | `static` modifier |
| `is_test` | name starts with `test`, or a `test`/`tests` path segment (already matches Maven's `src/test/java`) | Additionally: `@Test`, `@ParameterizedTest`, `@RepeatedTest`, `@TestFactory` (JUnit 4/5, TestNG); class names ending in `Test`, `Tests`, `IT` |
| `return_annotation` | `type` field | Already works for `method_declaration` |
| Normalized body | every child token | **Skip the `modifiers` node** in the token stream (it goes into metadata instead), so `public` vs `protected` or an extra annotation doesn't lower structural similarity |

### 4.5 False-positive control (the main Java-specific risk)

Java codebases contain a lot of structurally identical boilerplate that is not worth reporting:

- getters and setters (`return this.x;`, `this.x = x;`);
- trivial delegating constructors and builder `withX` methods;
- `equals`, `hashCode`, and `toString`, which usually come from IDEs.

Risks:

1. **Noise:** "utility" or "near-duplicate" clusters made of accessors.
2. **Starving real findings:** candidate generation is capped at 25,000 pairs per language (`MAX_CANDIDATES_PER_LANGUAGE`). An enterprise repo with 40k accessors could use up that budget.

Mitigations (proposed, validated in Phase 2):

- A **trivial-unit filter** for Java before fingerprinting. It drops units whose body is a single `return` of a field or parameter, or a single field assignment. The rule is conservative and covered by tests.
- Accessor-shaped clusters that survive the filter get the existing `utility_candidate` path, and the AI explanation text suggests records or Lombok rather than a helper.
- Generated code is already handled: `target/` and `build/` are excluded by default. Add `.gradle` to `DEFAULT_EXCLUDES`.
- Tune on real repos. Candidates: Apache Commons Lang, Spring PetClinic, a large multi-module project such as Apache Kafka (clients). Record precision and scan time, and adjust thresholds only for Java if needed.

### 4.6 Classification and output text

- Classification logic is language-neutral and needs no structural change.
- Review the explanation strings (`WHY`) for Python-only advice. For Java, test duplication should point to `@ParameterizedTest`, and strategy candidates to an interface plus implementations or an `enum`/`Map<Key, Handler>` registry.
- `LANGUAGE java` appears in AI output automatically.

### 4.7 Tests for Workstream A

- Unit tests in `native.rs` (following the existing `rust_*` tests):
  - renamed locals normalize the same way;
  - calls and members are preserved;
  - lambdas named through `variable_declarator`;
  - methods in anonymous classes don't pollute the parent's calls;
  - constructors, records, and compact constructors;
  - switch expressions and pattern matching;
  - text blocks;
  - annotations go to `decorators`, not tokens;
  - generics;
  - broken syntax skips only the broken unit;
  - the trivial-accessor filter.
- `tests/languages/duplicates.java`. Update the counts in `tests/multilingual.rs` (`files_scanned`, `functions_found`) and add `Language::Java` to the "must find duplicates" list and the cross-language isolation test.
- `tests/fixtures/provider_implementations/*.java`: a Java version of the Stripe/PayPal/Adyen demo, so Java gets the README's main use case.
- Benchmark: add a Java corpus case to `benches/pipeline_bench.rs`.

### 4.8 Workstream A deliverable

After Workstream A, **Java scanning ships through the existing channels** (PyPI, npm, NuGet, `cargo install`) with no Maven work. It can be released on its own as `0.4.0`.

---

## 5. Workstream B — Maven distribution

### 5.1 Options considered

| Option | How | Pros | Cons | Verdict |
|---|---|---|---|---|
| **1. Native binary in classifier artifacts + Java launcher/plugin** | Publish `dowsing-rod-native` with one classifier per platform. The plugin resolves only the matching one and runs it as a subprocess. | Same binary and behaviour as every other channel. The pattern is proven here (npm, NuGet) and used by protoc/grpc on Maven. Downloads one binary per platform, works through repository mirrors, no runtime downloads. A crash can't take down the Maven JVM. | Has to stage 5 (later 7) binaries per release. Platforms without a binary need a fallback. | ✅ **Recommended** |
| 2. JNI / Java FFM in-process library | Build a `cdylib` with a C ABI and load it from Java | Typed in-process API, no subprocess | Still needs per-platform natives. A native crash kills the build JVM. FFM needs Java 22+, and JNI is more code. Thread pool (rayon) runs inside the Maven JVM. | ❌ Not for v1. Reconsider only if someone needs an embedded API. |
| 3. Compile the engine to WebAssembly and run it on a pure-Java WASM runtime (e.g. Chicory) | `wasm32-wasi` build of the core with tree-sitter C grammars | A single platform-independent jar, no natives at all | Tree-sitter's C code needs a wasi-sdk toolchain. No rayon threads. Expected to be several times slower (must be measured). The runtime is young. | 🔬 Optional spike (Phase 7). A good fallback for platforms without a binary. |
| 4. Port the engine to Java | Rewrite with a Java parser | Pure Java | Months of work, two engines that drift apart, and the existing Python/npm/NuGet users get nothing from it | ❌ Rejected |

### 5.2 Proposed artifacts (Maven coordinates)

`groupId` is a decision (§10). It must be a namespace verified on Maven Central, e.g. `io.github.<github-owner>`. Placeholder used below: `io.github.OWNER`.

| artifactId | Packaging | Contents |
|---|---|---|
| `dowsing-rod-parent` | pom | Shared build config, `${revision}` version, Central metadata |
| `dowsing-rod-native` | jar × classifiers | One jar per platform, each holding only `native/<classifier>/dowsing-rod[.exe]` plus a `.sha256`. Classifiers follow the `os-maven-plugin` convention Java developers know from protoc: `linux-x86_64`, `linux-aarch_64`, `osx-x86_64`, `osx-aarch_64`, `windows-x86_64`, and later `linux-x86_64-musl`, `linux-aarch_64-musl`. |
| `dowsing-rod-java` | jar | Small Java 11 library: platform detection, native resolution, extraction and caching with checksum checks, process runner, a typed `ScanResult` model for the JSON schema, and an API `DowsingRod.scan(Path, ScanOptions)`. The plugin, the CLI jar, and a future Gradle plugin all share it. |
| `dowsing-rod-maven-plugin` | maven-plugin | The goals in §5.3. The `-maven-plugin` naming gives the short prefix `dowsing-rod:`. |
| `dowsing-rod-cli` *(optional, Phase 7)* | executable jar | `java -jar dowsing-rod-cli.jar scan .` and `jbang` support. It bundles all platform binaries, which makes it large, or resolves them as the plugin does. |

Jars are used rather than raw `exe` artifacts because some corporate repository managers and scanners block or quarantine bare executables. A jar is the most widely accepted packaging.

### 5.3 Plugin goals

| Goal | Default phase | Maps to | Purpose |
|---|---|---|---|
| `dowsing-rod:scan` | `verify` (when bound) | `dowsing-rod scan <root> …flags` | Scan with pom-level configuration, which overrides `dowsing-rod.toml` |
| `dowsing-rod:auto` | `verify` | `dowsing-rod auto <root>` | Config-file-only mode for hooks, CI, and agents. Fails without `dowsing-rod.toml`, as today. |
| `dowsing-rod:init` | none (CLI only) | `dowsing-rod init <root> [--force]` | Writes `dowsing-rod.toml` |
| `dowsing-rod:check` | `verify` | `scan --format json` + Java-side evaluation | **CI gate**: fails the build when findings exceed configured thresholds (see below) |
| `dowsing-rod:cache-clear` | none | `dowsing-rod cache clear` | Clears the cache |
| `dowsing-rod:help` | none | Generated | Lists goals and parameters |

Example usage:

```xml
<plugin>
  <groupId>io.github.OWNER</groupId>
  <artifactId>dowsing-rod-maven-plugin</artifactId>
  <version>0.4.0</version>
  <executions>
    <execution>
      <goals><goal>check</goal></goals>
    </execution>
  </executions>
  <configuration>
    <minSimilarity>0.85</minSimilarity>
    <format>ai</format>
    <maxTokens>4000</maxTokens>
    <excludes>
      <exclude>**/generated/**</exclude>
    </excludes>
    <failOnError>true</failOnError>
    <!-- check goal only -->
    <maxHighValueClusters>0</maxHighValueClusters>
  </configuration>
</plugin>
```

```bash
mvn dowsing-rod:scan -Ddowsing-rod.ai=true -Ddowsing-rod.maxTokens=2500
mvn dowsing-rod:init
mvn verify                 # runs `check` if bound
mvn verify -Ddowsing-rod.skip=true
```

### 5.4 Plugin behaviour details

**Parameters.** They map 1:1 to the current flags: `minSimilarity`, `normalization`, `format`, `ai`, `maxClusters`, `maxTokens`, `jobs`, `excludes`, `includes`, `useCache`, `failOnError`. Each has a `dowsing-rod.*` user property. Plugin-only parameters are `skip`, `outputFile`, `binaryPath`, `perModule`, and the `check` thresholds.

**Precedence matches today's rules.** Order: pom/`-D` value > `dowsing-rod.toml` > built-in defaults. The CHANGELOG has a fix for "explicit value equal to the default", and that needs care in Maven. Parameters therefore get **no Maven default values**. A `null` parameter means "not set" and is never passed as a flag, so a `dowsing-rod.toml` value wins unless the pom sets one explicitly.

**Multi-module reactors.** Cross-module duplication is one of the most valuable findings, so a scan per module would miss it. Default behaviour:

- run **once, at the execution root**, as an aggregator;
- scan the root `basedir`;
- skip the run in child modules;
- `perModule=true` gives a separate scan for each module.

**Project root and config.** The plugin passes the reactor root as the scan path. The core already walks up to the nearest `dowsing-rod.toml`, so subfolder runs and `auto` keep working.

**Output.**

- The child process's stdout and stderr stream into the Maven log.
- With `outputFile`, the report is also written to `target/dowsing-rod/report.{txt,json,jsonl}`, which CI can archive.
- Default stdout format stays `human`.

**Exit-code mapping.**

- The binary returns non-zero because of parse errors with `failOnError` → `MojoFailureException` (a build failure).
- The binary can't run or reports invalid configuration → `MojoExecutionException`.

**The `check` gate.** This is a new capability, added in the plugin only, so the core stays unchanged.

1. The plugin runs `--format json` and parses `statistics` and `clusters`.
2. It fails the build on any of: `maxHighValueClusters`, `maxClusters` with a minimum `value`, or `failOnClassifications` (for example `duplicate`).
3. It prints the human summary, so a CI failure explains itself.

**Thread safety.** The plugin is `threadSafe = true` for `mvn -T`. The core cache is per file and keyed by content.

**Version pinning.** At build time the plugin jar embeds its own version in a filtered `dowsing-rod.properties`. At run time it:

1. resolves `dowsing-rod-native:<same version>:<classifier>` through Maven Resolver, using the user's mirrors and offline mode;
2. checks the SHA-256;
3. runs `dowsing-rod version` once and **fails loudly on a version mismatch**.

### 5.5 Native resolution and execution (`dowsing-rod-java`)

**Platform detection.** It uses `os.name` and `os.arch`:

- maps them to the classifier;
- detects musl by the presence of `/lib/ld-musl-*`;
- falls back to a clear error that names the platform and suggests `binaryPath`. It works like `Program.cs`, which rejects musl today.

**Extraction.**

- Destination: `~/.m2/dowsing-rod/<version>/<classifier>/` (overridable). That is shared across projects and survives `mvn clean`.
- It writes to a temp file, then does an atomic move. This makes concurrent builds safe.
- It sets the executable bit with `PosixFilePermissions`.
- It doesn't extract into `/tmp`, because hardened CI agents often mount `/tmp` as `noexec`.

**Execution.** `ProcessBuilder` with an explicit argument list, so there is no shell and no quoting issues on Windows. The working directory is the project root, and stderr is drained on a separate thread so the process can't deadlock.

**`binaryPath` override.** Users can point at their own `dowsing-rod` binary: unsupported platforms, internally rebuilt binaries for regulated environments, or local engine development.

### 5.6 Repository layout (new)

```text
maven/
  pom.xml                         dowsing-rod-parent (${revision}, Central metadata, plugin mgmt)
  .mvn/wrapper/…                  Maven Wrapper; pins the Maven version (one place, like the toolchain action)
  dowsing-rod-native/pom.xml      attaches one classifier jar per staged binary that is present
  dowsing-rod-java/               launcher library + JSON model + unit tests
  dowsing-rod-maven-plugin/       mojos + maven-invoker-plugin integration tests (src/it/…)
scripts/
  package-maven.mjs               stage a built binary into maven/dowsing-rod-native/native/<classifier>/
```

As with NuGet's `Condition="Exists(...)"`, the native module only attaches binaries that are actually staged. A local developer can build and test with just their host binary.

---

## 6. Versioning and release

### 6.1 Keeping one version source

- `Cargo.toml` stays the **only** place the version is written.
- Maven poms use the CI-friendly `${revision}` property with `flatten-maven-plugin`, so the published poms contain concrete versions.
- CI passes `-Drevision="$(node scripts/version.mjs)"`.
- For local builds, add `node scripts/version.mjs --maven-config`. It writes a gitignored `maven/.mvn/maven.config` containing `-Drevision=<version>`, so `./mvnw verify` works without flags.
- A guard test in the plugin checks that the embedded version equals the native binary's `dowsing-rod version` output.

### 6.2 Version mapping

| Cargo | Maven | Notes |
|---|---|---|
| `0.4.0` | `0.4.0` | 1:1 |
| `0.4.0-rc.1` | `0.4.0-rc.1` | Maven `ComparableVersion` orders `alpha < beta < rc < release` |
| *(untagged main)* | `X.Y.Z-SNAPSHOT` *(optional)* | Only meaningful if `Cargo.toml` is bumped right after each release. Otherwise `0.3.0-SNAPSHOT` would sort *below* the released `0.3.0`. Decision in §10. |

**Maven Central releases are immutable.** Once published, a version can never be overwritten or deleted. That makes the existing `--check-tag` guard and the review gate even more important than they are for npm.

### 6.3 Maven Central requirements (Central Portal; OSSRH was retired in 2025)

- **Namespace verification** for the `groupId`: GitHub-based `io.github.<owner>`, or a domain verified by DNS.
- **POM metadata:** `name`, `description`, `url`, `licenses` (MIT), `developers`, `scm`.
- **Signatures:** a GPG `.asc` for every file.
- **Sources and javadoc jars** for every jar artifact. For `dowsing-rod-native`, minimal or empty ones are acceptable.
- **Checksums:** generated by the tooling.
- **Publishing:** with `central-publishing-maven-plugin`. All modules and native classifiers upload as **one deployment bundle**, so a release is atomic: the plugin can never be published without its natives.

### 6.4 CI/CD changes

| File | Change |
|---|---|
| `.github/actions/toolchain/action.yml` | Add a `java` input (`actions/setup-java`, Temurin, pinned). Maven is pinned through the wrapper. |
| `.github/workflows/build-native.yml` | Add `package: maven`. Add a `maven_classifier` column to the matrix and a "Stage native binary for Maven" step that calls `scripts/package-maven.mjs` and uploads `maven-native-<classifier>`. |
| `.github/workflows/ci.yml` | New **`java`** job with no Python: build the host binary → stage → `./mvnw -B verify` (unit tests + invoker ITs). Matrix: JDK 11/17/21/25 × Maven 3.6.3/3.9.x/4.0.x (a reduced matrix on PRs). |
| `.github/workflows/maven-release.yml` *(new)* | `workflow_dispatch` with a `publish` input, the same shape as `nuget-release.yml`. (1) `native` reuses `build-native.yml`. (2) `package` downloads all binaries, checks all classifiers are present, and runs `./mvnw verify` into a local staging repo. (3) `test-installed` runs on Linux/macOS/Windows and uses the staged repo as the only remote to run the plugin against `tests/languages` and a sample multi-module project. (4) `publish` needs a `v*` tag, the `maven-release` environment approval, and `version.mjs --check-tag`. It imports the GPG key and runs `./mvnw deploy -P release`. |
| `release.yml`, `npm-release.yml`, `nuget-release.yml` | **No change** |

**Secrets** (in the `maven-release` environment):

- `MAVEN_CENTRAL_USERNAME` and `MAVEN_CENTRAL_PASSWORD` (a Central Portal user token);
- `MAVEN_GPG_PRIVATE_KEY` and `MAVEN_GPG_PASSPHRASE`.

Central does not offer OIDC trusted publishing the way PyPI, npm, and NuGet do in this repo (re-check in Phase 0). These are therefore long-lived secrets and should be locked to the environment with required reviewers.

---

## 7. Impact analysis

### 7.1 On existing users and channels

| Area | Impact | Severity |
|---|---|---|
| CLI, flags, config keys | None. All additive. | None |
| Python / npm / NuGet packages | Same binary, now able to read Java. No packaging change. | None |
| **Mixed-language repos that contain `.java` files** | After upgrading, Java files are scanned too. There are more clusters, longer scans, and output differs from earlier runs. Users who pinned output in snapshots or set a CI gate will see changes. | **Medium.** Call it out in the CHANGELOG. Workaround: `exclude = ["**/*.java"]`. |
| JSON schema | `language` gains the value `"java"`, which is additive. Consumers that validate a closed enum would reject it. The Rust addition did not bump `schema_version`. | Low. Proposal: bump `schema_version` 1.1 → 1.2 for clarity (decision). |
| Cache | Cache keys include the tool version, so the upgrade invalidates them automatically. | None |
| Default excludes | Adding `.gradle` | Negligible |

### 7.2 On the codebase

| Area | Impact |
|---|---|
| `native.rs` | Grows by an estimated 250–400 lines. It is already 1.2k lines and the busiest file. **Option:** split the per-language tables (`unit_kind`, parameters, bindings) into `native/java.rs` modules during Phase 1. Higher merge-conflict risk with upstream (see the fork note in §10). |
| New Java code base | About 1.5–2.5k lines of Java plus integration tests under `maven/`. This brings a new language and toolchain for maintainers: Java and Maven skills are needed to review PRs. |
| Binary size | Every channel's binary grows by the Java grammar (an estimated low single-digit MB, to be measured in Phase 0). It affects npm, NuGet, and PyPI downloads. |
| Build time | `build-native.yml` is unchanged, apart from staging. The new Java CI job adds an estimated 5–10 minutes on PRs, and the release adds one 3-OS test job. |

### 7.3 Platform coverage

The binaries released today cover glibc Linux x64/arm64, macOS x64/arm64, and Windows x64. **Java CI often runs on Alpine images** (e.g. `maven:3.9-eclipse-temurin-21-alpine`), and musl is not supported. Without musl builds, the plugin fails on a large share of real Java pipelines.

**Recommendation:** add static `x86_64-unknown-linux-musl` and `aarch64-unknown-linux-musl` targets to `build-native.yml` as part of this work. That adds 2 matrix jobs. npm and NuGet benefit too, since they currently reject musl explicitly. Windows arm64 stays out of scope unless someone asks for it, and `binaryPath` remains the escape hatch.

### 7.4 Security and supply chain

- Natives arrive as signed Maven artifacts through the user's own repository manager. Nothing is downloaded from GitHub or elsewhere at run time, which is a property worth keeping.
- The plugin checks a SHA-256 that is embedded in the plugin jar at build time before it runs a binary. A tampered mirror artifact fails closed.
- The subprocess is run without a shell. Arguments are passed as a list.
- Release secrets are environment-scoped and require approval. Sigstore or a provenance attestation can be added as an optional extra.
- Privacy is unchanged: still fully offline and no telemetry.

### 7.5 Licensing

`tree-sitter-java` is MIT. Add it to `THIRD_PARTY_NOTICES.md`, and ship `LICENSE` plus `THIRD_PARTY_NOTICES.md` inside every Maven jar under `META-INF/`.

### 7.6 Maintenance

Each release now publishes to four registries. A Maven Central failure partway through can't be retried with the same version once it is published, so keep it review-gated and test the staged repo before publishing. Supported matrices to keep current:

- JDK: 11, 17, 21, 25;
- Maven: 3.6.3+ and 4.x.

---

## 8. Phased plan

Workstreams A and B can run **in parallel** after Phase 0. Effort estimates assume one engineer who knows Rust and Java.

### Phase 0 — Decisions and spikes *(2–3 days)*

- Resolve the open decisions in §10: groupId, fork vs upstream, Java/Maven baselines, musl, snapshots, schema bump.
- Pin the `tree-sitter-java` version and confirm it compiles with `tree-sitter 0.26` and `--locked`. Measure the growth in binary size.
- Parse spike: dump node kinds for a Java 21 corpus (records, sealed types, pattern switch, text blocks) and confirm the mapping table in §4.3.
- Start **Maven Central namespace verification** now, because it can take time. Generate the GPG key.
- **Exit:** decisions recorded in this document; spike notes; namespace verified.

### Phase 1 — Java frontend in the core *(5–8 days)* · Workstream A

- §4.2–4.4: register the language, add the grammar mapping, handle metadata, skip modifiers.
- §4.7: unit tests, `tests/languages/duplicates.java`, updated multilingual counts, the Java provider fixtures.
- Docs: README language list and "How it works", `docs/algorithms.md` (extensions, Java notes), `llms.txt`, `pyproject.toml` keywords, npm/NuGet READMEs, CHANGELOG.
- **Exit:** `cargo fmt/clippy/test --workspace --locked` pass; the Java demo produces a `strategy_candidate` like the Python one.

### Phase 2 — Java precision and performance tuning *(3–5 days)* · Workstream A

- §4.5: trivial-accessor filter, explanation text, `.gradle` exclude.
- Scan 3–5 real OSS Java repos. Record functions, clusters, top-10 precision (reviewed by hand), and scan time. Add a Java benchmark.
- **Exit:** a short precision report appended here. No accessor flood in the top clusters. Large-repo scan time comparable to the C# and TypeScript paths.
- **Release point:** ship `0.4.0` with Java through PyPI, npm, and NuGet (optional, but it gets feedback early).

### Phase 3 — Java runtime library and native artifacts *(3–4 days)* · Workstream B

- `maven/` parent with `${revision}` and flatten, the Maven Wrapper, `dowsing-rod-native` (classifier attachment), and `scripts/package-maven.mjs`.
- `dowsing-rod-java`: platform detection including musl, resolution and extraction with checksums, process runner, `ScanResult` model, unit tests.
- `scripts/version.mjs --maven-config`.
- **Exit:** `./mvnw verify` passes locally with the host binary; `DowsingRod.scan()` returns a parsed result for `tests/languages`.

### Phase 4 — Maven plugin *(5–7 days)* · Workstream B

- Goals `scan`, `auto`, `init`, `check`, `cache-clear`, `help` (§5.3–5.4), with a precedence model where unset means not passed.
- Aggregator-at-root behaviour, `perModule`, `skip`, `outputFile`, `binaryPath`, and the version-mismatch guard.
- Integration tests with `maven-invoker-plugin`:
  - single module;
  - multi-module with a duplicate *across* modules;
  - `auto` fails without config and passes after `init`;
  - `failOnError`;
  - `check` passes and fails;
  - `-Ddowsing-rod.skip`;
  - `-T 4`;
  - offline mode (`-o`) with a pre-resolved repo.
- **Exit:** invoker ITs pass on Linux, macOS, and Windows in CI.

### Phase 5 — CI and Maven Central release pipeline *(3–5 days)* · Workstream B

- §6.4: toolchain `java` input, `build-native.yml` maven staging, `ci.yml` java job, `maven-release.yml`. Add musl targets if approved.
- Dry run: `maven-release.yml` with `publish=false` on a tag, then a first real publish of an `-rc.1` to Central.
- **Exit:** the RC can be resolved from Central and runs in a clean sample project on all supported OSes, including Alpine if musl is approved.

### Phase 6 — Docs and launch *(1–2 days)*

- README: "With Maven" install section, plugin usage, `check` gate example, multi-module notes.
- `maven/README.md`: all goals, parameters, and user properties.
- `docs/platform-support.md`: JDK/Maven floors and the musl status. `llms.txt`: Maven commands for agents.
- CHANGELOG, the final release, and announcement notes.

### Phase 7 — Optional follow-ups *(effort per item)*

| Item | Value | Effort |
|---|---|---|
| Gradle plugin (`io.github.OWNER.dowsing-rod`) on the Gradle Plugin Portal, reusing `dowsing-rod-java` | Covers the other half of the Java ecosystem | 4–6 d |
| `dowsing-rod-cli` executable jar and a JBang catalog | `jbang dowsing-rod@OWNER scan .`, a `uvx`-like experience for Java | 2–3 d |
| SARIF output (core renderer) | GitHub code scanning and IDE annotations | 3–4 d |
| WASM/Chicory spike | A pure-JVM fallback for platforms without a native binary | 3–5 d spike |
| Kotlin frontend (`tree-sitter-kotlin`) | Mixed Java/Kotlin Maven projects | 5–8 d |
| Configurable cache directory (e.g. under `target/`) | Some teams want nothing written to the repo root | 1 d |

**Total for the core scope (Phases 0–6): about 22–34 engineer-days.**

---

## 9. Risks and mitigations

| # | Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|---|
| R1 | Accessor and boilerplate noise hides real findings or uses up the candidate cap | High | High | Trivial-unit filter, Phase 2 tuning on real repos, Java-specific tests |
| R2 | Alpine/musl CI agents can't run the binary | High | High | Add musl targets (§7.3); `binaryPath` escape hatch; clear error message |
| R3 | Maven Central namespace or signing setup delays the release | Medium | Medium | Start in Phase 0; RC dry run in Phase 5 |
| R4 | Immutable Central release published broken | Low | High | Atomic bundle, 3-OS test of the staged repo before publish, tag check, environment approval, `-rc` first |
| R5 | Precedence surprises (a pom default overriding `dowsing-rod.toml`) | Medium | Medium | Parameters have no defaults; ITs cover each precedence case |
| R6 | Corporate repository managers block or quarantine native content | Medium | Medium | Jar packaging (not `exe`), checksums, `binaryPath`, documentation |
| R7 | Merge conflicts with upstream in `native.rs` | Medium (fork) | Medium | Upstream the Java frontend early; consider splitting per-language modules |
| R8 | tree-sitter-java lags behind new Java syntax | Low | Low | Errors are per unit, so only the affected method is skipped; bump the grammar |
| R9 | Maven 4 API differences | Low | Medium | Build against the Maven 3 plugin API (works in Maven 4); add Maven 4 to the IT matrix |

---

## 10. Decisions needed before Phase 1

| # | Decision | Options | Recommendation |
|---|---|---|---|
| D1 | **Fork or upstream?** This repo is a fork of `38e9b0f8/dowsing-rod`. | (a) Contribute Java + Maven upstream; (b) maintain a fork-only Maven distribution | **(a) for Workstream A** at minimum, since language support benefits every channel and avoids conflicts. The Maven channel goes upstream as well if the maintainers agree, otherwise it lives in the fork. |
| D2 | **groupId / Central namespace** | `io.github.<owner>` (GitHub-verified) or a domain you control | Use the owner of whichever repo publishes (follows from D1) |
| D3 | **Java baseline for plugin and library** | 8 / 11 / 17 | **11.** Covers nearly all active Maven builds. 8 only if a known consumer needs it. |
| D4 | **Minimum Maven** | 3.6.3 / 3.9 | **3.6.3+**, tested on 3.9 and 4.0 |
| D5 | **musl in v1** | yes / later | **Yes** (R2) |
| D6 | **SNAPSHOT publishing** | yes / no | **No for v1.** Use RCs instead. Revisit together with a post-release version-bump convention. |
| D7 | **`schema_version` bump** for the new `"java"` value | keep 1.1 / bump to 1.2 | **Bump to 1.2** (a one-line change; clearer for strict consumers) |
| D8 | **`check` gate defaults** | off / fail on `high_value > 0` | **Off by default.** Users opt in to thresholds. |
| D9 | **Gradle in scope** | v1 / Phase 7 | **Phase 7** |

---

## Appendix A — File-by-file change inventory

**Workstream A (Rust core):**

```text
Cargo.toml, Cargo.lock                        + tree-sitter-java
crates/dowsing-core/Cargo.toml                + tree-sitter-java
crates/dowsing-core/src/language.rs           + Language::Java, .java, grammar, Display
crates/dowsing-core/src/native.rs             + Java units, params, bindings, scopes, calls, members,
                                                literals, control tokens, modifiers/annotations,
                                                is_public/is_static/is_test, trivial-unit filter, tests
crates/dowsing-core/src/discovery.rs          + ".gradle" default exclude
crates/dowsing-core/src/classification.rs     ~ Java-aware explanation text (review)
crates/dowsing-core/src/types.rs              ~ schema_version (if D7)
crates/dowsing-core/tests/multilingual.rs     ~ counts, Java assertions, cross-language test
benches/pipeline_bench.rs                     + Java case
tests/languages/duplicates.java               + new
tests/fixtures/provider_implementations/*.java + new
THIRD_PARTY_NOTICES.md, README.md, CHANGELOG.md, llms.txt, docs/algorithms.md,
pyproject.toml (keywords), npm/dowsing-rod/{README.md,package.json keywords},
nuget/dowsing-rod/README.md, nuget/Directory.Build.props (tags), .gitattributes (if needed)
```

**Workstream B (Maven):**

```text
maven/**                                      + new (parent, wrapper, native, java lib, plugin, ITs)
scripts/package-maven.mjs                     + new
scripts/version.mjs                           ~ --maven-config
.github/actions/toolchain/action.yml          ~ java input
.github/workflows/build-native.yml            ~ maven staging (+ musl targets if D5)
.github/workflows/ci.yml                      ~ java job
.github/workflows/maven-release.yml           + new
.gitignore                                    ~ maven/**/target, maven/.mvn/maven.config, staged natives
docs/platform-support.md, README.md, llms.txt ~ Maven sections
npm/dowsing-rod/bin/dowsing-rod.cjs, nuget/dowsing-rod/Program.cs  ~ only if musl is added
```

## Appendix B — Expected Java output (illustrative)

```text
C1 | strategy_candidate | signal=88% | confidence=84%
3 functions | duplicated~510 tokens | reduction~340 tokens | value=341
LANGUAGE java
MEMBERS
  payments/StripeGateway.java:StripeGateway::charge:12-31
  payments/PaypalGateway.java:PaypalGateway::charge:14-33
  payments/AdyenGateway.java:AdyenGateway::charge:11-30
COMMON
  validateAmount -> buildChargeRequest -> normalizeChargeResult
DIFF
  call target: stripeClient.charge vs paypalClient.charge vs adyenClient.charge
WHY
  High control-flow similarity with different call targets suggests a repeated pipeline;
  consider a PaymentGateway interface with provider implementations.
```
