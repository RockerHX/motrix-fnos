#!/usr/bin/env node
import { spawnSync } from 'node:child_process';
import process from 'node:process';

const result = spawnSync(
  process.execPath,
  ['--test', '--test-reporter=./scripts/verify/test-duration-reporter.mjs', 'scripts/tests/fpk-artifact.test.mjs'],
  {
    cwd: process.cwd(),
    env: { ...process.env, MOTRIX_REQUIRE_FPK_ARTIFACTS: '1' },
    stdio: 'inherit',
  },
);

process.exit(result.status ?? 1);
