#!/usr/bin/env node
import { chmodSync, copyFileSync, existsSync, mkdirSync } from 'node:fs';
import path from 'node:path';
import process from 'node:process';

const repoRoot = process.cwd();
const target = readOption('--target') ?? 'x86_64-unknown-linux-gnu';
const dryRun = process.argv.includes('--dry-run');
const outputName = readOption('--output-name') ?? 'aria2-next';
const destinationDir = path.join(repoRoot, 'packaging', 'fnos', 'app', 'bin');
const destinationPath = path.join(destinationDir, outputName);

const sourceMap = {
  'x86_64-unknown-linux-gnu': path.join(repoRoot, 'src-tauri', 'binaries', 'aria2-next-x86_64-unknown-linux-gnu'),
  'aarch64-unknown-linux-gnu': path.join(repoRoot, 'src-tauri', 'binaries', 'aria2-next-aarch64-unknown-linux-gnu'),
};

const sourcePath = sourceMap[target];
if (!sourcePath) {
  fail(`不支持的 sidecar 目标：${target}`);
}
if (!existsSync(sourcePath)) {
  fail(`缺少 sidecar 源文件：${sourcePath}`);
}

if (dryRun) {
  console.log(`${target} -> ${destinationPath}`);
  process.exit(0);
}

mkdirSync(destinationDir, { recursive: true });
copyFileSync(sourcePath, destinationPath);
chmodSync(destinationPath, 0o755);
console.log(`已放置 Aria2 Next sidecar：${sourcePath} -> ${destinationPath}`);

function readOption(name) {
  const index = process.argv.indexOf(name);
  if (index === -1) return undefined;
  return process.argv[index + 1];
}

function fail(message) {
  console.error(message);
  process.exit(1);
}
