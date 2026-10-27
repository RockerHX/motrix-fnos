import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { chmodSync, existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';

const verify = path.resolve(import.meta.dirname, '../verify/verify.mjs');
const cacheModule = new URL('../lib/verification-cache.mjs', import.meta.url).href;

test('验证摘要覆盖工作区内容、删除、新文件、权限、依赖、配置及工具链', (t) => {
  const f = fixture(t);
  const fingerprint = (env = {}) => {
    const result = f.run(['--input-type=module', '-e',
      `import { verificationFingerprint } from ${JSON.stringify(cacheModule)}; console.log(verificationFingerprint());`], env);
    assert.equal(result.status, 0, result.stderr);
    return result.stdout.trim();
  };
  let previous = fingerprint();
  assert.match(previous, /^[a-f0-9]{64}$/);
  for (const file of ['src/app.ts', 'src/app.spec.ts', 'pnpm-lock.yaml', 'vite.config.ts',
    '.env.production', 'node_modules/.modules.yaml', 'src/new.ts']) {
    f.write(file, 'changed');
    const next = fingerprint();
    assert.notEqual(next, previous, file);
    previous = next;
  }
  rmSync(path.join(f.root, 'src/app.ts'));
  assert.notEqual(fingerprint(), previous);
  previous = fingerprint();
  chmodSync(path.join(f.root, 'src/new.ts'), 0o755);
  assert.notEqual(fingerprint(), previous);
  previous = fingerprint();
  assert.notEqual(fingerprint({ CACHE_TOOL_VERSION: '2' }), previous);
  assert.notEqual(fingerprint({ RUSTFLAGS: '-C opt-level=1' }), previous);
  f.write('.cache/unrelated', 'ignored');
  assert.equal(fingerprint(), previous);
  f.git('add', '--all');
  f.git('commit', '--quiet', '--no-gpg-sign', '-m', 'fixture');
  assert.equal(fingerprint(), previous, '提交不改变工作区文件内容');
});

test('成功验证供后续打包和推送复用，手动 verify 仍强制执行', (t) => {
  const f = fixture(t);
  assert.equal(f.verify().status, 0);
  assert.ok(existsSync(f.cache));
  f.resetCalls();
  const reused = f.verify(['--reuse']);
  assert.equal(reused.status, 0, reused.stderr);
  assert.match(reused.stdout, /复用本地已通过/);
  assert.equal(f.calls().length, 0);
  assert.equal(f.verify().status, 0);
  assert.equal(f.calls().length, 11);
});

test('源码、Web 产物或记录变化都重新验证，CI 不复用本地结果', (t) => {
  const f = fixture(t);
  assert.equal(f.verify().status, 0);
  for (const mutate of [
    () => f.write('src/app.ts', 'new source'),
    () => f.write('dist/assets/app.js', 'tampered'),
    () => rmSync(path.join(f.root, 'dist'), { recursive: true }),
    () => f.write('.cache/verification.json', '{broken'),
  ]) {
    mutate();
    f.resetCalls();
    const result = f.verify(['--reuse']);
    assert.equal(result.status, 0, result.stderr);
    assert.equal(f.calls().length, 11);
  }
  f.resetCalls();
  assert.equal(f.verify(['--reuse'], { CI: '1' }).status, 0);
  assert.equal(f.calls().length, 11);
  assert.equal(existsSync(f.cache), false);
});

test('失败、快速检查和验证期间的修改不能生成成功记录', (t) => {
  const f = fixture(t);
  assert.equal(f.verify(['--quick']).status, 0);
  assert.equal(existsSync(f.cache), false);
  assert.equal(f.verify().status, 0);
  f.resetCalls();
  assert.notEqual(f.verify([], { CACHE_FAIL_AT: '3' }).status, 0);
  assert.equal(f.calls().length, 3);
  assert.equal(existsSync(f.cache), false);
  f.resetCalls();
  const result = f.verify([], { CACHE_MUTATE: '1' });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /验证期间源码或工具链发生变化/);
  assert.equal(existsSync(f.cache), false);
});

function fixture(t) {
  const root = mkdtempSync(path.join(os.tmpdir(), 'motrix-verification-cache-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const write = (file, content) => {
    mkdirSync(path.dirname(path.join(root, file)), { recursive: true });
    writeFileSync(path.join(root, file), content);
  };
  const git = (...args) => {
    const result = spawnSync('git', args, { cwd: root, encoding: 'utf8' });
    assert.equal(result.status, 0, result.stderr);
  };
  git('init', '--quiet');
  git('config', 'core.hooksPath', '/dev/null');
  git('config', 'user.name', 'Cache Test');
  git('config', 'user.email', 'cache@example.test');
  write('.gitignore', 'bin/\ndist/\n.cache/\nnode_modules/\n.env*\ncalls.jsonl\n');
  for (const file of ['src/app.ts', 'src/app.spec.ts', 'pnpm-lock.yaml', 'vite.config.ts']) write(file, 'original');
  const bin = path.join(root, 'bin');
  const log = path.join(root, 'calls.jsonl');
  const stub = `#!${process.execPath}
const fs = require('node:fs');
const path = require('node:path');
const call = [path.basename(process.argv[1]), ...process.argv.slice(2)];
if (['--version', '-vV'].includes(call[1])) { console.log('tool ' + (process.env.CACHE_TOOL_VERSION || '1')); process.exit(0); }
fs.appendFileSync(process.env.CACHE_LOG, JSON.stringify(call) + '\\n');
const count = fs.readFileSync(process.env.CACHE_LOG, 'utf8').trim().split('\\n').length;
if (count === Number(process.env.CACHE_FAIL_AT)) process.exit(19);
if (call[0] === 'pnpm' && call[2] === 'build') {
  fs.mkdirSync('dist/assets', { recursive: true });
  fs.writeFileSync('dist/index.html', '<html>verified</html>');
  fs.writeFileSync('dist/assets/app.js', 'verified');
  if (process.env.CACHE_MUTATE) fs.writeFileSync('src/app.ts', 'changed during verification');
}
`;
  for (const command of ['node', 'cargo', 'rustc', 'pnpm', 'sh']) {
    write(`bin/${command}`, stub);
    chmodSync(path.join(bin, command), 0o755);
  }
  git('add', '--all');
  git('commit', '--quiet', '--no-gpg-sign', '-m', 'base fixture');
  const run = (args, env = {}) => spawnSync(process.execPath, args, {
    cwd: root, encoding: 'utf8', timeout: 20_000,
    env: { ...process.env, CI: '', PATH: `${bin}${path.delimiter}${process.env.PATH ?? ''}`,
      npm_config_user_agent: 'pnpm/11.17.0', CACHE_LOG: log, ...env },
  });
  return {
    root, write, git, run, cache: path.join(root, '.cache/verification.json'),
    verify: (args = [], env = {}) => run([verify, ...args], env),
    resetCalls: () => rmSync(log, { force: true }),
    calls: () => existsSync(log) ? readFileSync(log, 'utf8').trim().split('\n').filter(Boolean).map(JSON.parse) : [],
  };
}
