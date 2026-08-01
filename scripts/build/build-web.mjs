#!/usr/bin/env node
import { performance } from 'node:perf_hooks';
import process from 'node:process';
import { runCommandWithProgress } from '../lib/command-progress.mjs';

const startedAt = performance.now();
await run('vue-tsc', ['--noEmit'], '前端类型检查', '正在执行：vue-tsc --noEmit');
await run('vite', ['build', '--logLevel', 'warn'], '前端生产构建', '正在执行：vite build --logLevel warn');
console.log(`Web UI 类型检查与生产构建通过，总耗时 ${((performance.now() - startedAt) / 1000).toFixed(2)}s。`);

async function run(command, args, title, initialDetail) {
  try {
    await runCommandWithProgress(command, args, {
      title,
      initialDetail,
      cwd: process.cwd(),
      env: process.env,
    });
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exit(error?.exitCode ?? 1);
  }
}
