# Dowsing Rod

Find structural refactoring opportunities in Python, JavaScript/JSX, TypeScript/TSX, C, C++, C#, Verilog, SystemVerilog, and VHDL.

```sh
dotnet tool install --global dowsing-rod
dowsing-rod scan . --ai --max-tokens 2500
```

This is a small .NET launcher for the Rust scanner. Parsing and analysis happen locally. It has no telemetry, install scripts, or runtime downloads.

Requires .NET 8+. Prebuilt binaries are included for macOS x64/arm64, Windows x64, and Linux glibc x64/arm64. musl/Alpine and Windows arm64 are not currently provided.
