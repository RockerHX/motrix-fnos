import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { cpSync, existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';

const repoRoot = path.resolve(import.meta.dirname, '../..');
const targets = ['x86_64-unknown-linux-gnu', 'aarch64-unknown-linux-gnu'];
const quick = [
  ['node', 'scripts/version/version-check.mjs'],
  ['cargo', 'fmt', '--manifest-path', 'server/Cargo.toml', '--all', '--', '--check'],
];
const full = [
  ...quick,
  ['pnpm', 'run', 'test:scripts'],
  ...['process-identity', 'startup-reconcile', 'lifecycle-stop', 'readiness']
    .map((name) => ['sh', `scripts/verify/test-fnos-${name}.sh`]),
  ['node', 'scripts/verify/run-rust-tests.mjs'],
  ['cargo', 'build', '--manifest-path', 'server/Cargo.toml'],
  ['pnpm', 'run', 'test:unit'],
  ['pnpm', 'run', 'build'],
];
const flows = [
  ['快速验证', 'verify/verify.mjs', ['--quick'], quick],
  ['完整验证', 'verify/verify.mjs', [], full],
  ['扩展验证', 'verify/verify-extended.mjs', [],
    ['verify', 'test:stability', 'test:e2e', 'test:aria2'].map((command) => ['pnpm', 'run', command])],
  ...['local', 'release'].map((mode) => [
    `${mode} 打包`, `build/package-${mode}.mjs`, [], [
      ['pnpm', 'run', mode === 'local' ? 'verify' : 'verify:extended', ...(mode === 'local' ? ['--reuse'] : [])],
      ['node', 'scripts/build/build-fpk-all.mjs', '--reuse-web-dist'],
      ['pnpm', 'run', 'verify:fpk'],
    ],
  ]),
  ...[[], ['--reuse-web-dist'], ['--prepare-only']].map((args) => [
    `双架构 ${args.join(' ')}`, 'build/build-fpk-all.mjs', args, [
      ['node', 'scripts/build/build-web-ui-fpk.mjs', ...(args.includes('--reuse-web-dist') ? ['--reuse-dist'] : [])],
      ...targets.map((target) => ['node', 'scripts/build/build-fpk.mjs', '--target', target,
        '--keep-dist', '--reuse-web-ui', ...(args.includes('--prepare-only') ? ['--prepare-only'] : [])]),
    ],
  ]),
  ['Web 构建', 'build/build-web.mjs', [], [
    ['vue-tsc', '--noEmit'], ['vite', 'build', '--logLevel', 'warn'],
  ]],
  ['FPK Web 构建', 'build/build-web-ui-fpk.mjs', [], [['pnpm', 'exec', 'vite', 'build', '--logLevel', 'warn']]],
  ['FPK Web 复用', 'build/build-web-ui-fpk.mjs', ['--reuse-dist'], []],
  ...[false, true].map((reuse) => [
    `单架构 FPK ${reuse ? '复用 UI' : '构建 UI'}`, 'build/build-fpk.mjs',
    ['--prepare-only', ...(reuse ? ['--reuse-web-ui'] : [])], [
      ['node', 'scripts/build/build-server-linux.mjs', '--target', targets[0]],
      ...(reuse ? [] : [['node', 'scripts/build/build-web-ui-fpk.mjs']]),
      ['node', 'scripts/build/stage-aria2-sidecar.mjs', '--target', targets[0]],
    ],
  ]),
  ['Rust 完整测试', 'verify/run-rust-tests.mjs', [], [['cargo', 'test', '--manifest-path', 'server/Cargo.toml']]],
  ['Rust 稳定性专项', 'verify/run-rust-tests.mjs', ['--stability'],
    [['cargo', 'test', '--manifest-path', 'server/Cargo.toml', '--test', 'runtime_stability', '--', '--ignored']]],
  ['独立文件系统定向入口', 'verify/run-filesystem-tests.mjs', [],
    ['tasks::files', 'runtime::file_cleanup', 'tasks::service::tests::permanent_delete',
      'tasks::service::tests::redownload_', 'tasks::service::tests::restore_'].map((filter) =>
      ['cargo', 'test', '--manifest-path', 'server/Cargo.toml', '--lib', filter, '--', '--nocapture'])],
];

for (const [title, script, args, expected] of flows) {
  test(`${title}：顺序正确，重复运行不叠加步骤，失败立即中止`, () => {
    const root = mkdtempSync(path.join(os.tmpdir(), 'motrix-verification-flow-'));
    try {
      const bin = path.join(root, 'bin');
      const log = path.join(root, 'calls.jsonl');
      mkdirSync(bin);
      mkdirSync(path.join(root, 'dist'));
      writeFileSync(path.join(root, 'dist/index.html'), '<html>fixture</html>');
      if (script === 'build/build-fpk.mjs') {
        const fpk = path.join(root, 'packaging/fnos');
        for (const entry of ['manifest.template', 'ICON.PNG', 'ICON_256.PNG', 'MotrixFNOS.sc', 'config', 'cmd', 'wizard', 'app/ui/config']) {
          const target = path.join(fpk, entry);
          mkdirSync(path.dirname(target), { recursive: true });
          cpSync(path.join(repoRoot, 'packaging/fnos', entry), target, { recursive: true });
        }
        for (const relative of [`server/target/${targets[0]}/release/motrix-fnos-server`, 'packaging/fnos/app/bin/aria2-next', 'packaging/fnos/app/ui/dist/index.html']) {
          mkdirSync(path.dirname(path.join(root, relative)), { recursive: true });
          writeFileSync(path.join(root, relative), 'fixture');
        }
        cpSync(path.join(fpk, 'ICON.PNG'), path.join(fpk, 'app/ui/dist/icon.png'));
      }
      const stub = `#!${process.execPath}
const fs = require('node:fs');
const path = require('node:path');
const call = [path.basename(process.argv[1]), ...process.argv.slice(2)];
fs.appendFileSync(process.env.FLOW_LOG, JSON.stringify(call) + '\\n');
const count = fs.readFileSync(process.env.FLOW_LOG, 'utf8').trim().split('\\n').length;
if (count === Number(process.env.FLOW_FAIL_AT)) process.exit(19);
if (call[0] === 'cargo' && call[1] === 'test') console.log('test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out;');
`;
      for (const command of ['node', 'cargo', 'pnpm', 'sh', 'vue-tsc', 'vite']) {
        writeFileSync(path.join(bin, command), stub, { mode: 0o755 });
      }
      const run = (failAt = '') => spawnSync(process.execPath, [path.join(repoRoot, 'scripts', script), ...args], {
        cwd: root,
        env: { ...process.env, PATH: `${bin}${path.delimiter}${process.env.PATH ?? ''}`,
          npm_config_user_agent: 'pnpm/11.17.0', FLOW_LOG: log, FLOW_FAIL_AT: String(failAt) },
        encoding: 'utf8', timeout: 15_000,
      });
      const calls = () => existsSync(log)
        ? readFileSync(log, 'utf8').trim().split('\n').filter(Boolean).map((line) => JSON.parse(line)) : [];
      for (let iteration = 0; iteration < 2; iteration += 1) {
        rmSync(log, { force: true });
        const result = run();
        assert.equal(result.status, 0, `${result.error ?? ''}\n${result.stdout}\n${result.stderr}`);
        assert.deepEqual(calls(), expected);
        if (script === 'build/build-web-ui-fpk.mjs') {
          assert.equal(readFileSync(path.join(root, 'packaging/fnos/app/ui/dist/index.html'), 'utf8'), '<html>fixture</html>');
        }
      }
      for (let failAt = 1; failAt <= expected.length; failAt += 1) {
        rmSync(log, { force: true });
        const result = run(failAt);
        assert.equal(result.error, undefined);
        assert.notEqual(result.status, null);
        assert.notEqual(result.status, 0, `${result.stdout}\n${result.stderr}`);
        assert.deepEqual(calls(), expected.slice(0, failAt));
      }
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  });
}
