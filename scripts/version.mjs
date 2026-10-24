// The project version, read from Cargo.toml's [workspace.package]: the only place it is set.
// Import `version`, or run `node scripts/version.mjs` to print it.
// `node scripts/version.mjs --check-tag v1.2.3` fails unless the tag matches the version.
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const cargo = readFileSync(new URL('../Cargo.toml', import.meta.url), 'utf8');
export const version = cargo.match(/^version = "([^"]+)"/m)?.[1];
if (!version) throw new Error('Cargo.toml has no [workspace.package] version');

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const [flag, tag] = process.argv.slice(2);
  if (flag === '--check-tag') {
    if (tag !== `v${version}`) {
      console.error(`release tag ${tag} does not match version v${version} in Cargo.toml`);
      process.exit(1);
    }
  } else {
    console.log(version);
  }
}
