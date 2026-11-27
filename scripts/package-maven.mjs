// Stage one already-built Rust binary for the dowsing-rod-native Maven classifier jar.
//
//   node scripts/package-maven.mjs <classifier|host> <binary> [output]
//
// Layout written under [output] (default: maven/dowsing-rod-native/native):
//   <classifier>/native/<classifier>/dowsing-rod[.exe]   the jar's content
//   <classifier>/native/<classifier>/dowsing-rod[.exe].sha256
//   <classifier>/META-INF/LICENSE, THIRD_PARTY_NOTICES.md
//   checksums/<classifier>.sha256                        embedded into dowsing-rod-java
import { mkdirSync, cpSync, chmodSync, statSync, readFileSync, writeFileSync, existsSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { resolve, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = fileURLToPath(new URL('../', import.meta.url));
const classifiers = new Map([
  ['osx-aarch_64', ''],
  ['osx-x86_64', ''],
  ['linux-aarch_64', ''],
  ['linux-x86_64', ''],
  ['linux-aarch_64-musl', ''],
  ['linux-x86_64-musl', ''],
  ['windows-x86_64', '.exe'],
]);

// The classifier of the machine running this script, for local development and CI.
function hostClassifier() {
  const arch = { x64: 'x86_64', arm64: 'aarch_64' }[process.arch];
  if (!arch) throw new Error(`unsupported architecture ${process.arch}`);
  if (process.platform === 'win32') return `windows-${arch}`;
  if (process.platform === 'darwin') return `osx-${arch}`;
  if (process.platform === 'linux') {
    const musl = !process.report.getReport().header.glibcVersionRuntime;
    return `linux-${arch}${musl ? '-musl' : ''}`;
  }
  throw new Error(`unsupported platform ${process.platform}`);
}

let [classifier, binary, output = join(root, 'maven/dowsing-rod-native/native')] = process.argv.slice(2);
if (classifier === 'host') classifier = hostClassifier();
if (!classifiers.has(classifier) || !binary || !existsSync(binary) || !statSync(binary).isFile()) {
  throw new Error(`Usage: node scripts/package-maven.mjs <host|${[...classifiers.keys()].join('|')}> <binary> [output]`);
}

const name = `dowsing-rod${classifiers.get(classifier)}`;
const base = resolve(output);
const directory = join(base, classifier, 'native', classifier);
mkdirSync(directory, { recursive: true });
const executable = join(directory, name);
cpSync(binary, executable);
chmodSync(executable, 0o755);

const digest = createHash('sha256').update(readFileSync(executable)).digest('hex');
writeFileSync(`${executable}.sha256`, `${digest}\n`);
mkdirSync(join(base, 'checksums'), { recursive: true });
writeFileSync(join(base, 'checksums', `${classifier}.sha256`), `${digest}\n`);

const meta = join(base, classifier, 'META-INF');
mkdirSync(meta, { recursive: true });
for (const file of ['LICENSE', 'THIRD_PARTY_NOTICES.md']) cpSync(join(root, file), join(meta, file));
console.log(`${classifier} ${digest} ${executable}`);
