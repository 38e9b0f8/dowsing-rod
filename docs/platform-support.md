# Platform support

The release workflows build the native CLI for macOS x64/arm64, Windows x64, Linux x64/arm64 (glibc), and Linux x64/arm64 (static musl).

- Linux glibc binaries target glibc 2.17 and are checked during the release build. They run on RHEL 8–10 and supported Ubuntu, Debian, Fedora, and SUSE releases.
- Linux musl binaries are statically linked and tested on Alpine. They also run on any other Linux.
- Intel macOS binaries target macOS 10.12; Apple Silicon binaries target macOS 11.
- Windows binaries use the static Microsoft C runtime.

The npm launcher requires Node 22+, and the NuGet tool requires a supported .NET 8 runtime, so those runtimes can impose a higher OS floor than the native scanner. Both pick the musl build automatically on musl systems. The Python wheels use the `manylinux2014` / glibc 2.17 baseline on Linux; musllinux wheels are not built yet.

## Maven

The Maven plugin and the `dowsing-rod-java` library need Java 8 or later and Maven 3.6.3 or later. They are tested on Java 8 and 21, with Maven 3.9 through the Maven Wrapper, on Linux, macOS, Windows, and Alpine.

The plugin resolves one `io.github.shkb42:dowsing-rod-native` classifier jar for the build machine. This is the same repository resolution that every other plugin uses, so mirrors, repository managers, and offline mode (`-o`) work unchanged:

| Classifier | Platform |
|---|---|
| `linux-x86_64`, `linux-aarch_64` | Linux, glibc 2.17+ |
| `linux-x86_64-musl`, `linux-aarch_64-musl` | Alpine and other musl Linux |
| `osx-x86_64`, `osx-aarch_64` | macOS 10.12+ (Intel), 11+ (Apple Silicon) |
| `windows-x86_64` | Windows x64 |

The binary is checked against a SHA-256 that ships inside the plugin's own jar before it runs. On any other platform, build the CLI from source and set `binaryPath`.

Windows arm64 is not packaged yet.
