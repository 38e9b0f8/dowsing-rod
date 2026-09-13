#!/usr/bin/env node
'use strict';
const { spawnSync } = require('node:child_process');
const { version, optionalDependencies } = require('../package.json');
const packageName = `dowsing-rod-native-${process.platform}-${process.arch}`;

if (!Object.hasOwn(optionalDependencies, packageName)) {
  console.error(`dowsing-rod: unsupported platform ${process.platform}/${process.arch}.`);
  process.exit(1);
}
if (process.platform === 'linux' && !process.report.getReport().header.glibcVersionRuntime) {
  console.error('dowsing-rod: the prebuilt Linux binaries require glibc; musl is not supported yet.');
  process.exit(1);
}
let executable;
try {
  const installed = require(`${packageName}/package.json`);
  if (installed.version !== version) throw new Error(`expected ${version}, found ${installed.version}`);
  executable = require.resolve(`${packageName}/bin/dowsing-rod${process.platform === 'win32' ? '.exe' : ''}`);
} catch (error) {
  console.error(`dowsing-rod: could not load ${packageName}@${version}. Reinstall with optional dependencies enabled.\n${error.message}`);
  process.exit(1);
}
const result = spawnSync(executable, process.argv.slice(2), { stdio: 'inherit', windowsHide: true });
if (result.error) {
  console.error(`dowsing-rod: ${result.error.message}`);
  process.exit(1);
}
if (result.signal) process.kill(process.pid, result.signal);
else process.exit(result.status ?? 1);
