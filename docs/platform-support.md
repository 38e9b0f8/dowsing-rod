# Platform support

The release workflows build the native CLI for macOS x64/arm64, Windows x64, and Linux x64/arm64.

- Linux binaries target glibc 2.17 and are checked during the release build. They run on RHEL 8–10 and supported Ubuntu, Debian, Fedora, and SUSE releases.
- Intel macOS binaries target macOS 10.12; Apple Silicon binaries target macOS 11.
- Windows binaries use the static Microsoft C runtime.

The npm launcher requires Node 22+, and the NuGet tool requires a supported .NET 8 runtime, so those runtimes can impose a higher OS floor than the native scanner. The Python wheels use the `manylinux2014` / glibc 2.17 baseline on Linux.

musl/Alpine and Windows arm64 are not packaged yet.
