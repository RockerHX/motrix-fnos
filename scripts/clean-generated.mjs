#!/usr/bin/env node
import { existsSync, readdirSync, rmSync } from 'node:fs';
import path from 'node:path';
import process from 'node:process';

const repoRoot = process.cwd();
const dryRun = process.argv.includes('--dry-run');

const generatedTargets = [
  'dist',
  'packaging/fnos/app/bin/aria2-next',
  'packaging/fnos/app/bin/motrix-fnos-server',
  'packaging/fnos/app/ui/dist',
  'packaging/fnos/.stage',
  'packaging/fnos/dist',
  'packaging/fnos/motrix.fnos.fpk',
];

const dsStoreFiles = collectDsStoreFiles(repoRoot);
const targets = [
  ...generatedTargets.map((target) => path.join(repoRoot, target)),
  ...dsStoreFiles,
];

let removedCount = 0;
for (const target of targets) {
  if (!existsSync(target)) {
    continue;
  }

  const relativeTarget = path.relative(repoRoot, target) || target;
  if (dryRun) {
    console.log(`[dry-run] remove ${relativeTarget}`);
    removedCount += 1;
    continue;
  }

  rmSync(target, { recursive: true, force: true });
  console.log(`removed ${relativeTarget}`);
  removedCount += 1;
}

if (removedCount === 0) {
  console.log(dryRun ? 'dry-run: no generated files found' : 'no generated files found');
}

function collectDsStoreFiles(dir) {
  const ignoredDirectories = new Set([
    '.git',
    '.codegraph',
    'node_modules',
    'target',
    'server/target',
  ]);
  const result = [];
  walk(dir);
  return result;

  function walk(currentDir) {
    const relativeDir = path.relative(repoRoot, currentDir);
    if (ignoredDirectories.has(relativeDir)) {
      return;
    }

    for (const entry of readdirSync(currentDir, { withFileTypes: true })) {
      const fullPath = path.join(currentDir, entry.name);
      if (entry.isDirectory()) {
        walk(fullPath);
        continue;
      }
      if (entry.isFile() && entry.name === '.DS_Store') {
        result.push(fullPath);
      }
    }
  }
}
