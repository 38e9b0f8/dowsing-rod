using System.Diagnostics;
using System.Runtime.InteropServices;

var rid = GetRuntimeIdentifier();
if (rid is null)
{
    Console.Error.WriteLine($"dowsing-rod: unsupported platform {RuntimeInformation.OSDescription}/{RuntimeInformation.OSArchitecture}.");
    return 1;
}

if (OperatingSystem.IsLinux() && RuntimeInformation.RuntimeIdentifier.Contains("musl", StringComparison.OrdinalIgnoreCase))
{
    Console.Error.WriteLine("dowsing-rod: the prebuilt Linux binaries require glibc; musl is not supported yet.");
    return 1;
}

var executable = Path.Combine(
    AppContext.BaseDirectory,
    "native",
    rid,
    OperatingSystem.IsWindows() ? "dowsing-rod.exe" : "dowsing-rod");

if (!File.Exists(executable))
{
    Console.Error.WriteLine($"dowsing-rod: no bundled binary is available for {rid}.");
    return 1;
}

if (!OperatingSystem.IsWindows())
{
    try
    {
        File.SetUnixFileMode(executable, UnixFileMode.UserRead | UnixFileMode.UserWrite | UnixFileMode.UserExecute |
                                          UnixFileMode.GroupRead | UnixFileMode.GroupExecute |
                                          UnixFileMode.OtherRead | UnixFileMode.OtherExecute);
    }
    catch (IOException)
    {
        // A read-only package cache can already preserve the executable mode.
    }
}

var startInfo = new ProcessStartInfo(executable)
{
    UseShellExecute = false,
};
foreach (var argument in args)
{
    startInfo.ArgumentList.Add(argument);
}

using var process = Process.Start(startInfo);
if (process is null)
{
    Console.Error.WriteLine("dowsing-rod: failed to start the bundled binary.");
    return 1;
}

await process.WaitForExitAsync();
return process.ExitCode;

static string? GetRuntimeIdentifier()
{
    return (RuntimeInformation.IsOSPlatform(OSPlatform.OSX), RuntimeInformation.IsOSPlatform(OSPlatform.Windows), RuntimeInformation.OSArchitecture) switch
    {
        (true, _, Architecture.Arm64) => "osx-arm64",
        (true, _, Architecture.X64) => "osx-x64",
        (_, true, Architecture.X64) => "win-x64",
        (_, _, Architecture.Arm64) when RuntimeInformation.IsOSPlatform(OSPlatform.Linux) => "linux-arm64",
        (_, _, Architecture.X64) when RuntimeInformation.IsOSPlatform(OSPlatform.Linux) => "linux-x64",
        _ => null,
    };
}
