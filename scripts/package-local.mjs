#!/usr/bin/env node
import { spawnSync } from 'node:child_process';
import { performance } from 'node:perf_hooks';
import process from 'node:process';

const startedAt = performance.now();
run('pnpm', ['run', 'verify']);
run('node', ['scripts/build-fpk-all.mjs', '--reuse-web-dist']);
run('pnpm', ['run', 'verify:fpk']);

console.log(`\n本地完整验证与双架构 FPK 构建通过，总耗时 ${formatDuration(performance.now() - startedAt)}。`);

function run(command, args) {
  const result = spawnSync(command, args, {
    cwd: process.cwd(),
    env: process.env,
    stdio: 'inherit',
  });
  if (result.status !== 0) {
    process.exit(result.status ?? 1);
  }
}

function formatDuration(milliseconds) {
  return `${(milliseconds / 1000).toFixed(2)}s`;
}
