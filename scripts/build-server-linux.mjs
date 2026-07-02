#!/usr/bin/env node
import { spawnSync } from 'node:child_process';
import { chmodSync, existsSync, mkdirSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import process from 'node:process';

const repoRoot = process.cwd();
const target = readOption('--target') ?? 'x86_64-unknown-linux-gnu';
const manifestPath = path.join(repoRoot, 'server', 'Cargo.toml');
const outputPath = path.join(repoRoot, 'server', 'target', target, 'release', 'motrix-fnos-server');
let env = {
  ...process.env,
  PATH: [path.join(os.homedir(), '.cargo', 'bin'), path.join(os.homedir(), '.local', 'bin'), process.env.PATH ?? ''].filter(Boolean).join(path.delimiter),
};

const isNativeLinuxX64 = process.platform === 'linux' && process.arch === 'x64' && target === 'x86_64-unknown-linux-gnu';
const args = isNativeLinuxX64
  ? ['build', '--manifest-path', manifestPath, '--release', '--target', target]
  : ['zigbuild', '--manifest-path', manifestPath, '--release', '--target', target];

if (!isNativeLinuxX64 && !hasCargoSubcommand('zigbuild', env)) {
  fail('未检测到 cargo-zigbuild。请先安装交叉构建依赖，例如：python3 -m pip install --user --break-system-packages cargo-zigbuild ziglang');
}
if (!isNativeLinuxX64) {
  env = ensureZig(env);
}

ensureRustTarget(target, env);
run('cargo', args, env);
console.log(`Linux server 构建完成：${outputPath}`);

function readOption(name) {
  const index = process.argv.indexOf(name);
  if (index === -1) return undefined;
  return process.argv[index + 1];
}

function hasCargoSubcommand(name, env) {
  const result = spawnSync('cargo', ['--list'], { cwd: repoRoot, env, encoding: 'utf8' });
  return result.status === 0 && result.stdout.includes(`    ${name}`);
}

function ensureZig(env) {
  if (which('zig', env)) {
    return env;
  }

  const pythonZig = which('python-zig', env) ?? findPythonZig(env);
  if (!pythonZig) {
    fail('未检测到 zig。请先安装交叉构建依赖，例如：python3 -m pip install --user --break-system-packages ziglang');
  }

  const wrapperDir = path.join(os.tmpdir(), 'motrix-fnos-zig-wrapper');
  const wrapper = path.join(wrapperDir, 'zig');
  mkdirSync(wrapperDir, { recursive: true });
  writeFileSync(wrapper, `#!/bin/sh\nexec "${pythonZig}" "$@"\n`);
  chmodSync(wrapper, 0o755);
  return {
    ...env,
    PATH: [wrapperDir, env.PATH ?? ''].filter(Boolean).join(path.delimiter),
  };
}

function findPythonZig(env) {
  const userBase = spawnSync('python3', ['-m', 'site', '--user-base'], { cwd: repoRoot, env, encoding: 'utf8' });
  if (userBase.status !== 0) {
    return null;
  }

  const candidate = path.join(userBase.stdout.trim(), 'bin', 'python-zig');
  return existsSync(candidate) ? candidate : null;
}

function ensureRustTarget(target, env) {
  if (!which('rustup', env)) {
    fail(`未检测到 rustup，无法确认 Rust target：${target}`);
  }

  const installed = spawnSync('rustup', ['target', 'list', '--installed'], { cwd: repoRoot, env, encoding: 'utf8' });
  if (installed.status !== 0) {
    fail(`读取已安装 Rust target 失败：${target}`);
  }

  if (installed.stdout.split(/\r?\n/).includes(target)) {
    return;
  }

  console.log(`未检测到 Rust target ${target}，准备执行 rustup target add ${target}`);
  run('rustup', ['target', 'add', target], env);
}

function which(command, env) {
  const result = spawnSync('sh', ['-lc', `command -v ${command}`], { cwd: repoRoot, env, encoding: 'utf8' });
  return result.status === 0 ? result.stdout.trim() : null;
}

function run(command, args, env) {
  const result = spawnSync(command, args, { cwd: repoRoot, env, stdio: 'inherit' });
  if (result.status !== 0) {
    process.exit(result.status ?? 1);
  }
}

function fail(message) {
  console.error(message);
  process.exit(1);
}
