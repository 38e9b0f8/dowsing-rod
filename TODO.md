# TODO: Java and Maven release

Branch `feature/java-maven` adds Java analysis, musl builds, and the Maven distribution, published from this fork as **`io.github.shkb42`**. The design is in [docs/java-maven-action-plan.md](docs/java-maven-action-plan.md). For now **only Maven is released**. PyPI is build-only (section 6), and npm and NuGet stay manual and opt-in.

## 1. Before the first public release

- [ ] **LGPL dependency.** `malachite*` (LGPL-3.0-only, pulled in by `rustpython-parser` for big integers) is statically linked into every shipped binary, including the Maven native jars. `deny.toml` allows it by name only, so CI passes. Decide one of these, then update `THIRD_PARTY_NOTICES.md`:
  - Accept it and meet the LGPL terms: list it in the notices and point to the source and object files needed for relinking.
  - Replace it, for example with a `rustpython-parser` build or fork that uses a permissive bigint crate.

## 2. GitHub repository setup (one-time, shkb42/dowsing-rod)

- [ ] *Actions* tab: enable workflows. They are disabled by default on forks, which is why nothing has run yet.
- [ ] *Settings → Actions → General*: set workflow permissions to **Read repository contents**, and do not let Actions create or approve pull requests.
- [ ] *Settings → Code security*: turn on the dependency graph, Dependabot alerts and security updates, secret scanning with push protection, and private vulnerability reporting.
- [ ] *Settings → Rules → Rulesets* for `main`: require a pull request, require status checks (CI and Security), block force pushes and deletions.
- [ ] Tag ruleset for `v*`: only you can create tags, and they cannot be updated or deleted.

## 3. Review and merge

- [ ] Push `feature/java-maven` and open a PR to `main`. CI, Security, and Dependency review must pass. None of these jobs has run on GitHub yet:
  - [ ] `maven` on Linux (JDK 8 and 21), Windows, and macOS
  - [ ] `musl`: static binary, npm launcher, and the Maven build on Alpine
  - [ ] Existing jobs: Rust and Python, glibc 2.17, npm and NuGet packed installs
  - [ ] `security.yml`: CodeQL, zizmor/actionlint, gitleaks, cargo-deny
- [ ] Fix or triage any Code scanning alerts, then merge.

## 4. Maven Central setup (one-time)

- [ ] Sign in at <https://central.sonatype.com> **with the GitHub account `shkb42`**. The `io.github.shkb42` namespace then shows as verified under *Namespaces*.
- [ ] Generate a user token (*Account → Generate User Token*).
- [ ] Create a dedicated signing key and publish its public half:
  ```bash
  gpg --full-generate-key                        # RSA 4096 or ed25519, with a passphrase and an expiry
  gpg --list-secret-keys --keyid-format=long
  gpg --keyserver keyserver.ubuntu.com --send-keys <KEYID>
  gpg --keyserver keys.openpgp.org --send-keys <KEYID>
  gpg --armor --export-secret-keys <KEYID>       # the value of MAVEN_GPG_PRIVATE_KEY
  ```
- [ ] *Settings → Environments → New environment* `maven-release`:
  - [ ] Required reviewers: you. Turn on *Prevent self-review* if anyone else can approve.
  - [ ] Deployment branches and tags: **selected tags only**, pattern `v*`.
  - [ ] Environment secrets (not repository secrets): `MAVEN_CENTRAL_USERNAME`, `MAVEN_CENTRAL_PASSWORD` (the token's two halves), `MAVEN_GPG_PRIVATE_KEY`, `MAVEN_GPG_PASSPHRASE`.

## 5. First release (a release candidate; Central releases can never be replaced)

- [ ] Set `version = "0.3.0-rc.1"` in `Cargo.toml`, run `cargo update --workspace`, update `CHANGELOG.md`, and merge through a PR.
- [ ] Dry run: *Actions → Maven release → Run workflow* on `main`, with **publish unchecked**. Every job must pass, including the staged-release tests on all seven platforms.
- [ ] `git tag -s v0.3.0-rc.1 -m "v0.3.0-rc.1" && git push origin v0.3.0-rc.1`
- [ ] *Maven release → Run workflow*, choose the **tag** `v0.3.0-rc.1`, check **publish**, then approve the `maven-release` deployment.
- [ ] Check the deployment on central.sonatype.com (*Deployments*). Search can take 30 minutes to a few hours to show it.
- [ ] Verify it as a user would: add the plugin to a fresh project and run `mvn verify` (README → Maven). Then check provenance: `gh attestation verify dowsing-rod-maven-plugin-0.3.0-rc.1.jar -R shkb42/dowsing-rod`.
- [ ] Repeat with the final version, `0.3.0`.
- [ ] After the first release, change `egress-policy: audit` to `block` in the publish job of `maven-release.yml`, allowing only the endpoints the harden-runner report lists.

## 6. Python / PyPI (deploy deferred)

`python-release.yml` builds and smoke-tests the wheels and sdist on every `v*` tag, but publishes nothing.

- [ ] Upgrade `pyo3` to 0.24.1 or later (RUSTSEC-2025-0020, RUSTSEC-2026-0177), then remove both ignores in `deny.toml`.
- [ ] Decide the PyPI project name. `dowsing-rod` belongs to upstream.
- [ ] Create the `pypi-release` environment (reviewers, `v*` tags only) and a PyPI trusted publisher for `shkb42/dowsing-rod`, workflow `python-release.yml`, environment `pypi-release`.
- [ ] Add the gated `publish` job sketched at the top of `python-release.yml`.

## 7. Other channels (only when releasing them)

- [ ] npm: the package names belong to upstream. Pick fork-owned names first, then bootstrap trusted publishing, including the musl packages `dowsing-rod-native-linux-x64-musl` and `dowsing-rod-native-linux-arm64-musl`. `npm-release.yml` expects 8 tarballs.
- [ ] NuGet: the same naming question applies. The tool package also carries `linux-musl-x64` and `linux-musl-arm64`.

## 8. Follow-ups (optional)

- [ ] Cut the IT fixture copies in `maven/dowsing-rod-maven-plugin/src/it` by sharing one fixture folder (about 50 fewer files)
- [ ] Pin the Docker images used in CI (`alpine`, `node`, `eclipse-temurin`, `manylinux2014`) by digest
- [ ] Gradle plugin on the Gradle Plugin Portal, reusing `dowsing-rod-java`
- [ ] `dowsing-rod-cli` executable jar and a JBang catalog (`jbang dowsing-rod@… scan .`)
- [ ] SARIF output for GitHub code scanning
- [ ] musllinux Python wheels
- [ ] Windows arm64 binaries
- [ ] Kotlin frontend (`tree-sitter-kotlin`)
- [ ] Configurable cache directory (for example under `target/`)
- [ ] Tune Java classification on real repositories (Commons Lang, Spring PetClinic, a large multi-module project) and record precision in the action plan
