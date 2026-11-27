// Run maven/release-smoke against a staged Maven repository in a clean folder and check the
// report, as a user's build would see the release. Used by .github/workflows/maven-release.yml.
//
//   node scripts/test-maven-release.mjs <staging-repo-dir> [maven-command]
import assert from 'node:assert/strict';
import { cpSync, mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { pathToFileURL, fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';
import { version } from './version.mjs';

const root = fileURLToPath(new URL('../', import.meta.url));
// The repository's Maven Wrapper pins the Maven version and works from any folder.
const wrapper = join(root, 'maven', process.platform === 'win32' ? 'mvnw.cmd' : 'mvnw');
const [staging, maven = wrapper] = process.argv.slice(2);
// Run the POSIX wrapper through sh, so it works without an executable bit.
const [command, prefix] = maven.endsWith('.cmd') || maven !== wrapper ? [maven, []] : ['sh', [wrapper]];
if (!staging) throw new Error('Usage: node scripts/test-maven-release.mjs <staging-repo-dir> [maven-command]');

// Outside the repository, whose pyproject.toml configures dowsing-rod for its own scans.
const work = mkdtempSync(join(tmpdir(), 'dowsing-maven-'));
try {
  cpSync(join(root, 'maven/release-smoke/pom.xml'), join(work, 'pom.xml'));
  cpSync(join(root, 'tests/fixtures/java_providers'), join(work, 'src/main/java/payments'), { recursive: true });
  const args = ['-B', '-ntp', `-Dmaven.repo.local=${join(work, 'm2')}`,
    `-Dstaging.url=${pathToFileURL(resolve(staging)).href}`, `-Ddowsing-rod.version=${version}`, 'verify'];
  const result = spawnSync(command, [...prefix, ...args], { cwd: work, encoding: 'utf8', shell: command.endsWith('.cmd'), maxBuffer: 32 * 1024 * 1024 });
  process.stdout.write(result.stdout);
  process.stderr.write(result.stderr);
  assert.equal(result.status, 0, `${maven} ${args.join(' ')} failed`);
  assert.match(result.stdout, new RegExp(`dowsing-rod ${version.replaceAll('.', '\\.')}: 3 files`));

  const report = JSON.parse(readFileSync(join(work, 'target/dowsing-rod.json'), 'utf8'));
  assert.equal(report.tool_version, version);
  assert.equal(report.statistics.files_scanned, 3);
  assert(report.functions.every(f => f.language === 'java'));
  const charge = report.clusters.find(c => c.function_indices.every(i => report.functions[i].function_name === 'charge'));
  assert(charge && charge.function_indices.length === 3, 'the three charge methods should cluster');
  console.log(`Maven release smoke test passed (${process.platform}-${process.arch})`);
} finally {
  rmSync(work, { recursive: true, force: true });
}
