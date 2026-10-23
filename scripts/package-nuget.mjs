// Stage one already-built Rust binary for the NuGet dotnet-tool package.
import { mkdirSync, cpSync, chmodSync, statSync } from 'node:fs';
import { resolve, join } from 'node:path';

const [rid, binary, output = 'dist/nuget-native'] = process.argv.slice(2);
const suffixes = new Map([
  ['osx-arm64', ''],
  ['osx-x64', ''],
  ['linux-arm64', ''],
  ['linux-x64', ''],
  ['win-x64', '.exe'],
]);

if (!suffixes.has(rid) || !binary || !statSync(binary).isFile()) {
  throw new Error('Usage: node scripts/package-nuget.mjs <osx-arm64|osx-x64|linux-arm64|linux-x64|win-x64> <binary> [output]');
}

const directory = resolve(output, rid);
mkdirSync(directory, { recursive: true });
const executable = join(directory, `dowsing-rod${suffixes.get(rid)}`);
cpSync(binary, executable);
chmodSync(executable, 0o755);
console.log(directory);
