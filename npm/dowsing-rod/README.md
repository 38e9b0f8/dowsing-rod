# Dowsing Rod

Find structural refactoring opportunities in Python, JavaScript/JSX, TypeScript/TSX, C, C++, C#, Verilog, SystemVerilog, and VHDL.

```sh
npx dowsing-rod scan . --ai --max-tokens 2500
npx dowsing-rod scan . --format json
```

A small Node launcher runs the Rust scanner. Parsing and analysis happen locally. No Python, compiler, language server, telemetry, install scripts, or runtime downloads are required. Native packages are installed through npm optional dependencies.

Requires Node 22+. Prebuilt binaries target macOS 10.12+ on Intel, macOS 11+ on Apple Silicon, Windows x64, and glibc 2.17+ on Linux x64/arm64. This covers RHEL 8–10, supported Ubuntu, Debian, Fedora, and SUSE releases. Node can impose a higher OS floor. musl/Alpine and Windows arm64 are not currently provided.

Clusters are isolated by language, including JavaScript versus TypeScript. HDL analysis covers functions, tasks, procedures, and `always`/`process` blocks. Findings are structural candidates, not proof of behavioral equivalence.

[Documentation and limitations](https://github.com/38e9b0f8/dowsing-rod#readme).
