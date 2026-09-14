# Dowsing Rod

Find structural refactoring opportunities in Python, JavaScript/JSX, TypeScript/TSX, C, C++, C#, Rust, Verilog, SystemVerilog, and VHDL.

```sh
dotnet tool install --global dowsing-rod
dowsing-rod scan . --ai --max-tokens 2500
```

This is a small .NET launcher for the Rust scanner. Parsing and analysis happen locally. It has no telemetry, install scripts, or runtime downloads.

Requires a supported .NET 8 runtime. Prebuilt binaries target macOS 10.12+ on Intel, macOS 11+ on Apple Silicon, Windows x64, and glibc 2.17+ on Linux x64/arm64. This covers RHEL 8–10 and current Ubuntu, Debian, Fedora, and SUSE releases. .NET can impose a higher OS floor. musl/Alpine and Windows arm64 are not currently provided.
