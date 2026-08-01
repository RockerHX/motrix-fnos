#!/usr/bin/env node
import { cpSync, existsSync, mkdirSync, readdirSync, rmSync } from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import process from 'node:process';

const repoRoot = process.cwd();
const sourceDir = path.join(repoRoot, 'dist');
const targetDir = path.join(repoRoot, 'packaging', 'fnos', 'app', 'ui', 'dist');
const reuseDist = process.argv.includes('--reuse-dist');

if (!reuseDist) {
  run('pnpm', ['run', 'build']);
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

function run(command, args) {
  const result = spawnSync(command, args, {
    cwd: repoRoot,
    stdio: 'inherit',
    env: process.env,
  });
  if (result.status !== 0) {
    process.exit(result.status ?? 1);
  }
}

function resetDir(dir) {
  mkdirSync(dir, { recursive: true });
  for (const entry of readdirSync(dir)) {
    rmSync(path.join(dir, entry), { recursive: true, force: true });
  }
}
