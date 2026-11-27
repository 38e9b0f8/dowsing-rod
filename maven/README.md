# Dowsing Rod for Maven and Java

These artifacts run the same native Rust scanner as the CLI, the npm package, and the NuGet tool. Analysis is local and offline. You need Java 8+ and Maven 3.6.3+; nothing else.

| Artifact (`io.github.shkb42:…`) | What it is |
|---|---|
| `dowsing-rod-maven-plugin` | The Maven plugin (goal prefix `dowsing-rod`) |
| `dowsing-rod-java` | A dependency-free Java 8+ API used by the plugin, for your own tools |
| `dowsing-rod-native:<classifier>` | The executable for one platform: `linux-x86_64`, `linux-aarch_64`, `linux-x86_64-musl`, `linux-aarch_64-musl`, `osx-x86_64`, `osx-aarch_64`, `windows-x86_64` |
| `dowsing-rod-parent` | Shared build configuration |

All versions equal the Rust version in `Cargo.toml`.

## Quick start

Try it on any Maven project without editing the POM:

```bash
mvn io.github.shkb42:dowsing-rod-maven-plugin:scan
```

Add it to the build, so `mvn verify` reports on every run and CI can gate on it:

```xml
<plugin>
  <groupId>io.github.shkb42</groupId>
  <artifactId>dowsing-rod-maven-plugin</artifactId>
  <version>0.3.0</version>
  <executions>
    <execution>
      <goals><goal>check</goal></goals>
      <configuration>
        <!-- Remove these two to report without ever failing the build. -->
        <maxClustersFound>0</maxClustersFound>
        <minRefactoringValue>100</minRefactoringValue>
      </configuration>
    </execution>
  </executions>
</plugin>
```

Everyday commands, once the plugin is declared:

```bash
mvn dowsing-rod:scan                                         # human report in the build log
mvn dowsing-rod:scan -Ddowsing-rod.ai=true -Ddowsing-rod.maxTokens=4000   # compact output for AI agents
mvn dowsing-rod:scan -Ddowsing-rod.format=json -Ddowsing-rod.outputFile=target/dowsing-rod.json
mvn dowsing-rod:init && mvn dowsing-rod:auto                 # keep settings in dowsing-rod.toml
mvn verify -Ddowsing-rod.skip=true                           # skip once
```

Don't use `mvn -q`: it hides the report, which is logged at INFO level. More recipes are in the main [README](../README.md#maven).

## Plugin goals

| Goal | Default phase | CLI equivalent |
|---|---|---|
| `scan` | `verify` | `dowsing-rod scan <dir> …` |
| `auto` | `verify` | `dowsing-rod auto <dir>`: uses `dowsing-rod.toml` only, and fails without it |
| `check` | `verify` | `scan --format json`, then thresholds (below) |
| `init` | none | `dowsing-rod init <dir>` (`-Ddowsing-rod.force=true`) |
| `cache-clear` | none | `dowsing-rod cache clear <dir>` |
| `help` | none | Parameter reference (`mvn dowsing-rod:help -Ddetail=true`) |

`scan`, `auto`, `init`, and `cache-clear` also work without a `pom.xml`, in the current folder: `mvn io.github.shkb42:dowsing-rod-maven-plugin:scan`.

### Scan options (`scan`, `check`)

Every option is unset by default. An unset option is not passed to the scanner, so `dowsing-rod.toml` decides it. A set option always overrides the file, even when it equals the built-in default.

| Parameter | Property | CLI flag |
|---|---|---|
| `minSimilarity` | `dowsing-rod.minSimilarity` | `--min-similarity` |
| `normalization` | `dowsing-rod.normalization` | `--normalization` |
| `format` (`human`, `ai`, `json`, `jsonl`) | `dowsing-rod.format` | `--format` / `--ai` |
| `ai` | `dowsing-rod.ai` | `--ai` |
| `maxClusters` | `dowsing-rod.maxClusters` | `--max-clusters` |
| `maxTokens` | `dowsing-rod.maxTokens` | `--max-tokens` |
| `jobs` | `dowsing-rod.jobs` | `--jobs` |
| `excludes` / `includes` | `dowsing-rod.excludes` / `dowsing-rod.includes` (comma-separated) | `--exclude` / `--include` |
| `useCache` | `dowsing-rod.useCache` | `false` → `--no-cache` |
| `failOnError` | `dowsing-rod.failOnError` | `--fail-on-error` / `--no-fail-on-error` |

### Plugin options (all goals)

| Parameter | Property | Default | |
|---|---|---|---|
| `skip` | `dowsing-rod.skip` | `false` | Skip the goal |
| `outputFile` | `dowsing-rod.outputFile` | none | Also write the report to a file (JSON reports go only to the file) |
| `perModule` | `dowsing-rod.perModule` | `false` | Run in every module, for that module's folder |
| `path` | `dowsing-rod.path` | project folder | What to scan |
| `binaryPath` | `dowsing-rod.binaryPath` | none | Run your own `dowsing-rod` build instead of `dowsing-rod-native` |

### `check` thresholds

All thresholds are off unless set. Without any threshold, `check` only reports.

| Parameter | Fails when |
|---|---|
| `maxClustersFound` | more clusters are reported than this |
| `maxHighValueClusters` | the scan's `high_value_clusters` exceeds this |
| `failOnClassifications` | a cluster is, for example, `duplicate` or `strategy_candidate` |
| `minRefactoringValue` | (filter) only clusters with at least this `refactoring_value` count for the two above |

## Multi-module builds

By default a goal runs **once**, in the reactor's top-level project. It scans that whole folder, so duplication between modules is found, and the other modules log that they are covered. When only a child module declares the plugin, the goal runs in that module for its own folder. `perModule=true` runs every module separately.

## How the binary is found

1. `binaryPath`, if set. The plugin warns if its version differs from the plugin's.
2. Otherwise `dowsing-rod-native:<plugin version>:<classifier>` is resolved through the build's plugin and project repositories, like any artifact, so mirrors, proxies, and `-o` work.
3. It is extracted next to that jar in the local repository (`…/dowsing-rod-native/<version>/extracted/`), or under `target/dowsing-rod/` if the local repository is read-only.
4. Before it runs, its SHA-256 must equal the one recorded in `dowsing-rod-java` at build time, and `dowsing-rod version` must report the plugin's version. Anything else fails the build.

Unsupported platforms fail with a message that points to `binaryPath`.

## Java API

```java
import io.github.dowsingrod.*;

DowsingRod rod = DowsingRod.locate();     // -Ddowsing-rod.binaryPath, DOWSING_ROD_BINARY, or a native jar on the classpath
ScanResult result = rod.scan(Paths.get("."), new ScanOptions().minSimilarity(0.85).useCache(false));
for (ScanResult.Cluster cluster : result.clusters()) {
    System.out.println(cluster.id() + " " + cluster.classification() + " " + cluster.members());
}
```

`locate()` needs `dowsing-rod-native` with your platform's classifier on the classpath, or an explicit binary.

## Developing

```bash
# from the repository root
cargo build -p dowsing-cli --release
node scripts/package-maven.mjs host target/release/dowsing-rod      # .exe on Windows
node scripts/version.mjs --maven-config                             # version from Cargo.toml
cd maven && ./mvnw install                                          # mvnw.cmd on Windows
```

`install` runs the unit tests and the integration tests in `dowsing-rod-maven-plugin/src/it`. These are real Maven builds that resolve the native jar from a private local repository. A local build packages only the staged host binary. The release workflow (`.github/workflows/maven-release.yml`) stages all seven, tests the staged repository on every platform, then signs and publishes it. Add `-Ddowsing.requireNative=true` to fail, rather than skip, the end-to-end tests when no binary is staged.
