# Dowsing Rod

Find structural refactoring opportunities in Python, JavaScript/JSX, TypeScript/TSX, C, C++, C#, Verilog, SystemVerilog, and VHDL.

```sh
npx dowsing-rod scan . --ai --max-tokens 2500
npx dowsing-rod scan . --format json
```

A small Node launcher runs the Rust scanner. Parsing and analysis happen locally. No Python, compiler, language server, telemetry, install scripts, or runtime downloads are required. Native packages are installed through npm optional dependencies.

Requires Node 22+. Prebuilt binaries: macOS x64/arm64, Windows x64, Linux glibc x64 (2.35+) and arm64 (2.39+). musl/Alpine and Windows arm64 are not currently provided.

Clusters are isolated by language, including JavaScript versus TypeScript. HDL analysis covers functions, tasks, procedures, and `always`/`process` blocks. Findings are structural candidates, not proof of behavioral equivalence.

See the repository README for documentation and limitations.
