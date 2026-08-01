#!/usr/bin/env node
import { existsSync, rmSync } from 'node:fs';
import path from 'node:path';
import process from 'node:process';

const repoRoot = process.cwd();
const targetRoot = path.join(repoRoot, 'server', 'target');
const dryRun = process.argv.includes('--dry-run');
const quiet = process.argv.includes('--quiet');

if (!existsSync(targetRoot)) {
  log(dryRun ? '[dry-run] server/target not found' : 'server/target not found');
  process.exit(0);
}

removeTarget(targetRoot);
log(`${dryRun ? '[dry-run] would remove' : 'removed'} server/target`);

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
