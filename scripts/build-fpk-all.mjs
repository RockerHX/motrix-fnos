#!/usr/bin/env node
import { mkdirSync, readdirSync, rmSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import path from 'node:path';
import process from 'node:process';

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

const webArgs = ['scripts/build-web-ui-fpk.mjs'];
if (reuseWebDist) {
  webArgs.push('--reuse-dist');
}
console.log(`\n==> ${reuseWebDist ? '复用已验证的 Web UI 构建' : '构建 FPK Web UI'}`);
run('node', webArgs);

for (const target of targets) {
  const args = ['scripts/build-fpk.mjs', '--target', target, '--keep-dist', '--reuse-web-ui'];
  forwardOption(args, '--service-port');
  forwardOption(args, '--fnpack');
  if (prepareOnly) {
    args.push('--prepare-only');
  }

  console.log(`\n==> 构建 FPK 目标：${target}`);
  run('node', args);
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

function run(command, args) {
  const result = spawnSync(command, args, { cwd: repoRoot, stdio: 'inherit', env: process.env });
  if (result.status !== 0) {
    process.exit(result.status ?? 1);
  }
}
