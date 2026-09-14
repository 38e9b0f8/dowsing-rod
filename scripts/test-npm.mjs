// Exercise packed artifacts, installed offline in a clean directory, through the real CLI.
import assert from 'node:assert/strict';
import { mkdtempSync, readFileSync, writeFileSync, mkdirSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';
const root = fileURLToPath(new URL('../', import.meta.url));
const stage = resolve(process.argv[2] || 'dist/npm');
const platform = `${process.platform}-${process.arch}`;
const work = mkdtempSync(join(tmpdir(), 'dowsing-npm-'));
const npm = process.platform === 'win32' ? 'npm.cmd' : 'npm';
function run(command, args, cwd = work, expected = 0) {
  const result = spawnSync(command, args, { cwd, encoding: 'utf8', shell: command.endsWith('.cmd'), env: { ...process.env, npm_config_cache: join(work, 'cache') }, maxBuffer: 8 * 1024 * 1024 });
  assert.equal(result.status, expected, `${command} ${args.join(' ')}\n${result.error || ''}\n${result.stderr}`);
  return result.stdout;
}
try {
  const tarballs = [];
  for (const name of [`dowsing-rod-native-${platform}`, 'dowsing-rod']) {
    const packed = JSON.parse(run(npm, ['pack', join(stage, name), '--json', '--ignore-scripts']));
    assert(packed[0].files.some(f => f.path.startsWith('bin/')));
    tarballs.push(join(work, packed[0].filename));
  }
  writeFileSync(join(work, 'package.json'), '{"private":true}\n');
  run(npm, ['install', '--offline', '--ignore-scripts', '--no-audit', '--no-fund', ...tarballs]);
  const launcher = join(work, 'node_modules/dowsing-rod/bin/dowsing-rod.cjs');
  const version = JSON.parse(readFileSync(join(stage, 'dowsing-rod/package.json'))).version;
  assert.equal(run(process.execPath, [launcher, 'version']).trim(), `dowsing-rod ${version}`);
  assert.match(run(npm, ['exec', '--offline', '--', 'dowsing-rod', 'version']), new RegExp(version.replaceAll('.', '\\.')));
  const fixture = join(root, 'tests/languages');
  const result = JSON.parse(run(process.execPath, [launcher, 'scan', fixture, '--format', 'json', '--no-cache', '--fail-on-error']));
  assert.equal(result.statistics.files_scanned, 10);
  assert.equal(result.functions.length, 37);
  assert.equal(result.parse_errors.length, 0);
  assert.deepEqual(
    new Set(result.functions.map(f => f.language)),
    new Set(['javascript', 'typescript', 'c', 'cpp', 'csharp', 'rust', 'verilog', 'system_verilog', 'vhdl']),
  );
  assert.match(run(process.execPath, [launcher, 'scan', fixture, '--ai', '--max-tokens', '1000', '--no-cache']), /LANGUAGE/);
  const lines = run(process.execPath, [launcher, 'scan', fixture, '--format', 'jsonl', '--no-cache']).trim().split('\n').map(JSON.parse);
  assert.equal(lines[0].type, 'header'); assert(lines.length > 1);
  for (const line of lines.slice(1)) assert.equal(new Set(line.functions.map(f => f.language)).size, 1);
  mkdirSync(join(work, 'source'));
  writeFileSync(join(work, 'source/bad.ts'), 'function broken( {');
  run(process.execPath, [launcher, 'scan', join(work, 'source'), '--fail-on-error'], work, 1);
  run(process.execPath, [launcher, '--unknown-flag'], work, 2);
  // Missing optional binary must fail clearly, never silently use another version.
  rmSync(join(work, `node_modules/dowsing-rod-native-${platform}`), { recursive: true });
  run(process.execPath, [launcher, 'version'], work, 1);
  console.log(`npm packed-install tests passed (${platform})`);
} finally {
  rmSync(work, { recursive: true, force: true });
}
