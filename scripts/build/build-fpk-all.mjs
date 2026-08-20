#!/usr/bin/env node
import { mkdirSync, readdirSync, rmSync } from 'node:fs';
import path from 'node:path';
import process from 'node:process';
import { runCommandWithProgress } from '../lib/command-progress.mjs';

const repoRoot = process.cwd();
const outputDir = path.join(repoRoot, 'packaging', 'fnos', 'dist');
const prepareOnly = process.argv.includes('--prepare-only');
const reuseWebDist = process.argv.includes('--reuse-web-dist');
const targets = [
  'x86_64-unknown-linux-gnu',
  'aarch64-unknown-linux-gnu',
];

if (!prepareOnly) {
  resetDir(outputDir);
}

const webArgs = ['scripts/build/build-web-ui-fpk.mjs'];
if (reuseWebDist) {
  webArgs.push('--reuse-dist');
}
console.log(`\n==> ${reuseWebDist ? '复用已验证的 Web UI 构建' : '构建 FPK Web UI'}`);
await run('node', webArgs, reuseWebDist ? '复用已验证的 Web UI 构建' : '构建 FPK Web UI');

for (const target of targets) {
  const args = ['scripts/build/build-fpk.mjs', '--target', target, '--keep-dist', '--reuse-web-ui'];
  forwardOption(args, '--service-port');
  forwardOption(args, '--fnpack');
  if (prepareOnly) {
    args.push('--prepare-only');
  }

  console.log(`\n==> 构建 FPK 目标：${target}`);
  await run('node', args, `构建 FPK 目标：${target}`, '准备目标构建');
}

if (prepareOnly) {
  console.log('\nFPK 双架构预组装验证完成，已跳过 fnpack build');
} else {
  console.log(`\nFPK 双架构构建完成，输出目录：${outputDir}`);
}

function resetDir(dir) {
  mkdirSync(dir, { recursive: true });
  for (const entry of readdirSync(dir)) {
    rmSync(path.join(dir, entry), { recursive: true, force: true });
  }
}

function forwardOption(args, name) {
  const index = process.argv.indexOf(name);
  if (index === -1) return;
  const value = process.argv[index + 1];
  if (!value) {
    console.error(`缺少 ${name} 参数值`);
    process.exit(1);
  }
  args.push(name, value);
}

async function run(command, args, title, initialDetail = title) {
  try {
    await runCommandWithProgress(command, args, { title, initialDetail, cwd: repoRoot, env: process.env });
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exit(error?.exitCode ?? 1);
  }
}
