#!/usr/bin/env node
import { existsSync, readdirSync, rmSync } from 'node:fs';
import path from 'node:path';
import process from 'node:process';

const repoRoot = process.cwd();
const targetRoot = path.join(repoRoot, 'server', 'target');
const dryRun = process.argv.includes('--dry-run');
const incrementalOnly = process.argv.includes('--incremental');
const quiet = process.argv.includes('--quiet');

if (!existsSync(targetRoot)) {
  log(dryRun ? '[dry-run] server/target not found' : 'server/target not found');
  process.exit(0);
}

if (incrementalOnly) {
  const incrementalDirs = collectIncrementalDirs(targetRoot);
  if (incrementalDirs.length === 0) {
    log(dryRun ? '[dry-run] no Rust incremental cache found' : 'no Rust incremental cache found');
    process.exit(0);
  }

  for (const dir of incrementalDirs) {
    removeTarget(dir);
  }
  log(`${dryRun ? '[dry-run] would remove' : 'removed'} ${incrementalDirs.length} Rust incremental cache director${incrementalDirs.length === 1 ? 'y' : 'ies'}`);
  process.exit(0);
}

removeTarget(targetRoot);
log(`${dryRun ? '[dry-run] would remove' : 'removed'} server/target`);

function collectIncrementalDirs(root) {
  const result = [];
  walk(root);
  return result;

  function walk(currentDir) {
    for (const entry of readdirSync(currentDir, { withFileTypes: true })) {
      if (!entry.isDirectory()) {
        continue;
      }

      const fullPath = path.join(currentDir, entry.name);
      if (entry.name === 'incremental') {
        result.push(fullPath);
        continue;
      }
      walk(fullPath);
    }
  }
}

function removeTarget(target) {
  const relativeTarget = path.relative(repoRoot, target);
  if (dryRun) {
    log(`[dry-run] remove ${relativeTarget}`);
    return;
  }

  rmSync(target, { recursive: true, force: true });
  log(`removed ${relativeTarget}`);
}

function log(message) {
  if (!quiet) {
    console.log(message);
  }
}
