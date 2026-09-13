// Stage npm packages from already-built binaries. No downloads, install hooks, or dependencies.
import { readFileSync, mkdirSync, cpSync, chmodSync, writeFileSync, statSync } from 'node:fs';
import { resolve, join } from 'node:path';
import { fileURLToPath } from 'node:url';
const root = fileURLToPath(new URL('../', import.meta.url));
const manifest = JSON.parse(readFileSync(join(root, 'npm/dowsing-rod/package.json')));
const platforms = JSON.parse(readFileSync(join(root, 'npm/platforms.json')));
const [platform, binary, output = 'dist/npm'] = process.argv.slice(2);
const destination = resolve(output);
for (const file of ['Cargo.toml', 'pyproject.toml']) {
  const version = readFileSync(join(root, file), 'utf8').match(/^version = "([^"]+)"/m)?.[1];
  if (version !== manifest.version) throw new Error(`${file} version differs from npm version`);
}
const nugetVersion = readFileSync(join(root, 'nuget/Directory.Build.props'), 'utf8').match(/<Version>([^<]+)<\/Version>/)?.[1];
if (nugetVersion !== manifest.version) throw new Error('NuGet version differs from npm version');
if (Object.keys(manifest.optionalDependencies).length !== Object.keys(platforms).length) throw new Error('Platform manifest mismatch');
for (const name of Object.keys(platforms)) {
  if (manifest.optionalDependencies[`dowsing-rod-native-${name}`] !== manifest.version) throw new Error(`Version mismatch for ${name}`);
}
function documentation(directory) {
  for (const file of ['LICENSE', 'THIRD_PARTY_NOTICES.md']) cpSync(join(root, file), join(directory, file));
}
if (platform === 'launcher') {
  const directory = join(destination, 'dowsing-rod');
  mkdirSync(directory, { recursive: true });
  cpSync(join(root, 'npm/dowsing-rod'), directory, { recursive: true });
  documentation(directory);
  chmodSync(join(directory, 'bin/dowsing-rod.cjs'), 0o755);
  console.log(directory);
} else {
  const spec = platforms[platform];
  if (!spec || !binary || !statSync(binary).isFile()) throw new Error('Usage: node scripts/package-npm.mjs <platform|launcher> <binary|-> [output]');
  const directory = join(destination, `dowsing-rod-native-${platform}`);
  mkdirSync(join(directory, 'bin'), { recursive: true });
  const executable = join(directory, 'bin', `dowsing-rod${spec.os === 'win32' ? '.exe' : ''}`);
  cpSync(binary, executable); chmodSync(executable, 0o755);
  const pkg = { name: `dowsing-rod-native-${platform}`, version: manifest.version, description: `Native ${platform} executable for dowsing-rod`, license: manifest.license, os: [spec.os], cpu: [spec.cpu], ...(spec.libc ? { libc: [spec.libc] } : {}), files: ['bin/', 'LICENSE', 'THIRD_PARTY_NOTICES.md'], publishConfig: { access: 'public' } };
  writeFileSync(join(directory, 'package.json'), JSON.stringify(pkg, null, 2) + '\n');
  documentation(directory);
  console.log(directory);
}
