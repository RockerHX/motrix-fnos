#!/usr/bin/env node
import { spawnSync } from 'node:child_process';
import os from 'node:os';
import path from 'node:path';
import process from 'node:process';

const repoRoot = process.cwd();
const target = 'x86_64-unknown-linux-gnu';
const manifestPath = path.join(repoRoot, 'server', 'Cargo.toml');
const outputPath = path.join(repoRoot, 'server', 'target', target, 'release', 'motrix-fnos-server');
const env = {
  ...process.env,
  PATH: [path.join(os.homedir(), '.cargo', 'bin'), process.env.PATH ?? ''].filter(Boolean).join(path.delimiter),
};

const isNativeLinuxX64 = process.platform === 'linux' && process.arch === 'x64';
const command = 'cargo';
const args = isNativeLinuxX64
  ? ['build', '--manifest-path', manifestPath, '--release', '--target', target]
  : ['zigbuild', '--manifest-path', manifestPath, '--release', '--target', target];

if (!isNativeLinuxX64 && !hasCargoSubcommand('zigbuild', env)) {
  fail(
    '未检测到 cargo-zigbuild。请先安装交叉构建依赖，例如：python3 -m pip install --user cargo-zigbuild ziglang'
  );
}

run(command, args, env);
console.log(`Linux x86_64 server 构建完成：${outputPath}`);

function hasCargoSubcommand(name, env) {
  const result = spawnSync('cargo', ['--list'], {
    cwd: repoRoot,
    env,
    encoding: 'utf8',
  });
  return result.status === 0 && result.stdout.includes(`    ${name}`);
}

function run(command, args, env) {
  const result = spawnSync(command, args, {
    cwd: repoRoot,
    env,
    stdio: 'inherit',
  });
  if (result.status !== 0) {
    process.exit(result.status ?? 1);
  }
}

function fail(message) {
  console.error(message);
  process.exit(1);
}
