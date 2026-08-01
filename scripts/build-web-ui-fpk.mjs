#!/usr/bin/env node
import { cpSync, existsSync, mkdirSync, readdirSync, rmSync } from 'node:fs';
import path from 'node:path';
import process from 'node:process';
import { runCommandWithProgress } from './command-progress.mjs';

const repoRoot = process.cwd();
const sourceDir = path.join(repoRoot, 'dist');
const targetDir = path.join(repoRoot, 'packaging', 'fnos', 'app', 'ui', 'dist');
const reuseDist = process.argv.includes('--reuse-dist');

if (!reuseDist) {
  await run('pnpm', ['run', 'build']);
}
if (!existsSync(path.join(sourceDir, 'index.html'))) {
  console.error(`Web UI 构建结果无效：${sourceDir} 缺少 index.html`);
  process.exit(1);
}
resetDir(targetDir);
cpSync(sourceDir, targetDir, { recursive: true });
console.log(`${reuseDist ? '已验证的 Web UI' : 'Web UI'} 已同步到 ${targetDir}`);

if (!existsSync(path.join(targetDir, 'index.html'))) {
  console.error('Web UI 同步失败：缺少 index.html');
  process.exit(1);
}

async function run(command, args) {
  try {
    await runCommandWithProgress(command, args, {
      title: '构建 FPK Web UI',
      cwd: repoRoot,
      env: process.env,
    });
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exit(error?.exitCode ?? 1);
  }
}

function resetDir(dir) {
  mkdirSync(dir, { recursive: true });
  for (const entry of readdirSync(dir)) {
    rmSync(path.join(dir, entry), { recursive: true, force: true });
  }
}
