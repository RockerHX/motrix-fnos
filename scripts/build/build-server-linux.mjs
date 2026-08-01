#!/usr/bin/env node
import { spawnSync } from 'node:child_process';
import { chmodSync, existsSync, mkdirSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import process from 'node:process';
import { cargoProgressDetail, runCommandWithProgress } from '../lib/command-progress.mjs';

const repoRoot = process.cwd();
const target = readOption('--target') ?? 'x86_64-unknown-linux-gnu';
const cargoTarget = resolveCargoTarget(target);
const rustTarget = stripGlibcSuffix(cargoTarget);
const manifestPath = path.join(repoRoot, 'server', 'Cargo.toml');
const outputPath = path.join(repoRoot, 'server', 'target', rustTarget, 'release', 'motrix-fnos-server');
let env = {
  ...process.env,
  PATH: [path.join(os.homedir(), '.cargo', 'bin'), path.join(os.homedir(), '.local', 'bin'), process.env.PATH ?? ''].filter(Boolean).join(path.delimiter),
};

const args = ['zigbuild', '--manifest-path', manifestPath, '--release', '--target', cargoTarget];

if (!hasCargoSubcommand('zigbuild', env)) {
  fail('未检测到 cargo-zigbuild。请先安装交叉构建依赖，例如：python3 -m pip install --user --break-system-packages cargo-zigbuild ziglang');
}
env = ensureZig(env);
env = appendRustFlags(env, ['-A', 'linker_messages']);

await ensureRustTarget(rustTarget, env);
await run('cargo', args, env, `编译 Linux server：${target}`);
console.log(`Linux server 构建完成：${outputPath}（glibc baseline: ${cargoTarget}）`);

function readOption(name) {
  const index = process.argv.indexOf(name);
  if (index === -1) return undefined;
  return process.argv[index + 1];
}

function hasCargoSubcommand(name, env) {
  if (which(`cargo-${name}`, env)) {
    return true;
  }

  const result = spawnSync('cargo', [name, '--help'], { cwd: repoRoot, env, encoding: 'utf8' });
  return result.status === 0;
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

async function ensureRustTarget(target, env) {
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
  await run('rustup', ['target', 'add', target], env, `安装 Rust target：${target}`);
}

function resolveCargoTarget(target) {
  if (!target.endsWith('-linux-gnu')) {
    return target;
  }

  const baseline = resolveGlibcBaseline(target);
  return `${target}.${baseline}`;
}

function resolveGlibcBaseline(target) {
  const archKey = target.startsWith('aarch64-') ? 'ARM64' : target.startsWith('x86_64-') ? 'X64' : null;
  const baseline =
    readOption('--glibc-baseline')
    ?? (archKey ? process.env[`MOTRIX_FNOS_GLIBC_BASELINE_${archKey}`] : undefined)
    ?? process.env.MOTRIX_FNOS_GLIBC_BASELINE
    ?? '2.36';

  if (!/^\d+\.\d+$/.test(baseline)) {
    fail(`无效的 glibc baseline：${baseline}`);
  }
  return baseline;
}

function stripGlibcSuffix(target) {
  return target.replace(/\.\d+\.\d+$/, '');
}

function which(command, env) {
  const result = spawnSync('sh', ['-lc', `command -v ${command}`], { cwd: repoRoot, env, encoding: 'utf8' });
  return result.status === 0 ? result.stdout.trim() : null;
}

function appendRustFlags(env, flags) {
  const existing = env.RUSTFLAGS?.trim();
  return {
    ...env,
    RUSTFLAGS: [...(existing ? [existing] : []), ...flags].join(' '),
  };
}

async function run(command, args, env, title) {
  try {
    await runCommandWithProgress(command, args, {
      title,
      initialDetail: '准备 Cargo 交叉编译',
      activity: cargoProgressDetail,
      cwd: repoRoot,
      env,
    });
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exit(error?.exitCode ?? 1);
  }
}

function fail(message) {
  console.error(message);
  process.exit(1);
}
