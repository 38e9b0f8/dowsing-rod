// Stage one already-built Rust binary for the NuGet dotnet-tool package.
import { readFileSync, mkdirSync, cpSync, chmodSync, statSync } from 'node:fs';
import { resolve, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = fileURLToPath(new URL('../', import.meta.url));
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

const versions = [
  readFileSync(join(root, 'Cargo.toml'), 'utf8').match(/^version = "([^"]+)"/m)?.[1],
  readFileSync(join(root, 'pyproject.toml'), 'utf8').match(/^version = "([^"]+)"/m)?.[1],
  readFileSync(join(root, 'nuget/Directory.Build.props'), 'utf8').match(/<Version>([^<]+)<\/Version>/)?.[1],
];
if (new Set(versions).size !== 1 || versions.includes(undefined)) throw new Error('Package versions differ');

const directory = resolve(output, rid);
mkdirSync(directory, { recursive: true });
const executable = join(directory, `dowsing-rod${suffixes.get(rid)}`);
cpSync(binary, executable);
chmodSync(executable, 0o755);
console.log(directory);
