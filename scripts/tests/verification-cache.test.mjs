import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { chmodSync, existsSync, mkdirSync, mkdtempSync, readFileSync, renameSync, rmSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';

const verify = path.resolve(import.meta.dirname, '../verify/verify.mjs');
const cacheModule = new URL('../lib/verification-cache.mjs', import.meta.url).href;
const groups = ['scripts', 'rust', 'frontend'];
const light = [['node', 'scripts/version/version-check.mjs'], ['node', '--test', 'scripts/tests/logging-docs.test.mjs']];

test('文件用途决定验证范围，普通文档不失效，运行资源和跨模块依赖不能遗漏', (t) => {
  const f = fixture(t);
  let previous = f.fingerprint();
  for (const value of Object.values(previous)) assert.match(value, /^[a-f0-9]{64}$/);
  for (const [file, affected] of [
    ['README.md', []], ['docs/maintenance.md', []], ['docs/api-contract.md', []], ['docs/logo.svg', []],
    ['CHANGELOG.md', ['scripts', 'frontend']], ['src/app.ts', ['scripts', 'frontend']],
    ['src/app.spec.ts', ['scripts', 'frontend']], ['src/runtime.md', ['scripts', 'frontend']],
    ['public/logo.svg', ['scripts', 'frontend']], ['vite.config.ts', ['scripts', 'frontend']],
    ['server/src/app.rs', ['scripts', 'rust']], ['server/tests/data.txt', ['scripts', 'rust']],
    ['server/Cargo.lock', ['scripts', 'rust']], ['packaging/fnos/ICON.PNG', ['scripts']],
    ['packaging/fnos/config/resource', ['scripts']],
    ...['common.sh', 'install_callback', 'reset-web-auth'].map((name) => [`packaging/fnos/cmd/${name}`, ['scripts', 'rust']]),
    ['assets/aria2/aria2-next-linux', ['scripts', 'rust']], ['scripts/tests/fixture.txt', ['scripts']],
    ['.env.production', ['frontend']], ['node_modules/.modules.yaml', ['scripts', 'frontend']],
    ['pnpm-lock.yaml', groups], ['scripts/build/build-web.mjs', groups], ['unknown.bin', groups], ['unknown.md', groups], ['docs/check.mjs', groups],
  ]) {
    f.write(file, 'changed');
    const next = f.fingerprint();
    assert.deepEqual(groups.filter((group) => next[group] !== previous[group]), affected, file);
    previous = next;
  }
});

test('摘要覆盖删除、权限、重命名、新文件与工具链，提交本身不失效', (t) => {
  const f = fixture(t);
  let previous = f.fingerprint();
  rmSync(path.join(f.root, 'src/app.ts'));
  assert.notDeepEqual(f.fingerprint(), previous);
  f.write('src/new.ts', 'new');
  previous = f.fingerprint();
  chmodSync(path.join(f.root, 'src/new.ts'), 0o755);
  assert.notDeepEqual(f.fingerprint(), previous);
  previous = f.fingerprint();
  assert.notDeepEqual(f.fingerprint({ CACHE_TOOL_VERSION: '2' }), previous);
  assert.equal(f.fingerprint({ RUSTFLAGS: '-C opt-level=1' }).frontend, previous.frontend);
  assert.notEqual(f.fingerprint({ RUSTFLAGS: '-C opt-level=1' }).rust, previous.rust);
  assert.notEqual(f.fingerprint({ RUSTFLAGS: '-C opt-level=1' }).scripts, previous.scripts);
  assert.equal(f.fingerprint({ VITE_MODE: 'changed' }).rust, previous.rust);
  assert.notEqual(f.fingerprint({ VITE_MODE: 'changed' }).frontend, previous.frontend);
  f.write('.cache/unrelated', 'ignored');
  assert.deepEqual(f.fingerprint(), previous);
  f.write('docs/placeholder.md', 'documentation');
  renameSync(path.join(f.root, 'src/new.ts'), path.join(f.root, 'docs/moved.md'));
  assert.notDeepEqual(f.fingerprint(), previous);
  previous = f.fingerprint();
  f.git('add', '--all');
  f.git('commit', '--quiet', '--no-gpg-sign', '-m', 'fixture');
  assert.deepEqual(f.fingerprint(), previous, '提交不改变工作区文件内容');
});

test('成功验证供后续打包和推送复用，手动 verify 仍强制执行', (t) => {
  const f = fixture(t);
  assert.equal(f.verify().status, 0);
  assert.ok(existsSync(f.cache));
  f.resetCalls();
  const reused = f.verify(['--reuse']);
  assert.equal(reused.status, 0, reused.stderr);
  assert.match(reused.stdout, /复用本地已通过/);
  assert.deepEqual(f.calls(), light);
  f.resetCalls();
  assert.equal(f.verify().status, 0);
  assert.equal(f.calls().length, 11);
});

test('日志环境变量的添加、变化和移除不影响验证复用', (t) => {
  const f = fixture(t);
  const previous = f.fingerprint({ RUST_LOG: undefined });
  assert.equal(f.verify([], { RUST_LOG: undefined }).status, 0);
  for (const RUST_LOG of ['debug', 'trace', undefined]) {
    assert.deepEqual(f.fingerprint({ RUST_LOG }), previous);
    f.resetCalls();
    const result = f.verify(['--reuse'], { RUST_LOG });
    assert.equal(result.status, 0, result.stderr);
    assert.deepEqual(f.calls(), light);
  }
});

test('文档只执行轻量检查，前后端和打包改动只重跑受影响的验证组', (t) => {
  const f = fixture(t);
  assert.equal(f.verify().status, 0);
  for (const [file, executed, reused] of [
    ['README.md', [], groups],
    ['docs/maintenance.md', [], groups],
    ['src/app.ts', ['scripts', 'frontend'], ['rust']],
    ['server/src/app.rs', ['scripts', 'rust'], ['frontend']],
    ['packaging/fnos/ICON.PNG', ['scripts'], ['rust', 'frontend']],
    ['CHANGELOG.md', ['scripts', 'frontend'], ['rust']],
    ['unknown.bin', groups, []],
  ]) {
    f.write(file, 'changed');
    f.resetCalls();
    const result = f.verify(['--reuse']);
    assert.equal(result.status, 0, result.stderr);
    for (const [group, label] of [['scripts', '脚本检查'], ['rust', '后端验证'], ['frontend', '前端验证']]) {
      assert.ok(result.stdout.includes(`[${executed.includes(group) ? '执行' : '复用'}] ${label}`), file);
    }
    assert.equal(f.calls().some((call) => call[0] === 'cargo'), !reused.includes('rust'), file);
    assert.equal(f.calls().some((call) => call[2] === 'test:unit'), !reused.includes('frontend'), file);
    assert.equal(f.calls().some((call) => call[2] === 'test:scripts'), !reused.includes('scripts'), file);
    if (executed.length === 0) assert.deepEqual(f.calls(), light);
  }
});

test('Web 输出只使前端失效，损坏和旧版记录完整验证，CI 不复用', (t) => {
  const f = fixture(t);
  assert.equal(f.verify().status, 0);
  for (const [mutate, count] of [
    [() => f.write('dist/assets/app.js', 'tampered'), 4],
    [() => rmSync(path.join(f.root, 'dist'), { recursive: true }), 4],
    [() => f.write('.cache/verification.json', '{broken'), 11],
    [() => f.write('.cache/verification.json', JSON.stringify({ inputs: f.fingerprint(), web: 'old' })), 11],
  ]) {
    mutate();
    f.resetCalls();
    const result = f.verify(['--reuse']);
    assert.equal(result.status, 0, result.stderr);
    assert.equal(f.calls().length, count);
  }
  f.resetCalls();
  assert.equal(f.verify(['--reuse'], { CI: '1' }).status, 0);
  assert.equal(f.calls().length, 11);
  assert.equal(existsSync(f.cache), false);
});

test('一组验证失败保留无关结果，不能沿用失败组旧记录', (t) => {
  const f = fixture(t);
  assert.equal(f.verify().status, 0);
  const rust = JSON.parse(readFileSync(f.cache, 'utf8')).groups.rust;
  f.write('src/app.ts', 'changed');
  f.resetCalls();
  assert.notEqual(f.verify(['--reuse'], { CACHE_FAIL_AT: '2' }).status, 0);
  assert.deepEqual(JSON.parse(readFileSync(f.cache, 'utf8')).groups, { rust });
  f.resetCalls();
  assert.equal(f.verify(['--reuse']).status, 0);
  assert.equal(f.calls().some((call) => call[0] === 'cargo'), false);
  assert.equal(f.calls().some((call) => call[2] === 'test:unit'), true);
});

test('全部复用时文档断言仍能阻止通过，验证期间 Web 变化也须中止', (t) => {
  const f = fixture(t);
  assert.equal(f.verify().status, 0);
  f.resetCalls();
  assert.notEqual(f.verify(['--reuse'], { CACHE_FAIL_AT: '2' }).status, 0);
  assert.deepEqual(f.calls(), light);
  f.write('server/src/app.rs', 'changed');
  f.resetCalls();
  const result = f.verify(['--reuse'], { CACHE_TAMPER_WEB: '1' });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /验证期间 Web 产物发生变化/);
  assert.equal(JSON.parse(readFileSync(f.cache, 'utf8')).groups.rust, undefined);
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
  for (const file of ['src/app.ts', 'src/app.spec.ts', 'server/src/app.rs', 'pnpm-lock.yaml', 'vite.config.ts']) write(file, 'original');
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
if (process.env.CACHE_TAMPER_WEB && call[1] === 'scripts/verify/run-rust-tests.mjs') fs.writeFileSync('dist/assets/app.js', 'changed during verification');
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
    fingerprint: (env = {}) => {
      const result = run(['--input-type=module', '-e',
        `import { verificationFingerprint } from ${JSON.stringify(cacheModule)}; console.log(JSON.stringify(verificationFingerprint()));`], env);
      assert.equal(result.status, 0, result.stderr);
      return JSON.parse(result.stdout);
    },
    resetCalls: () => rmSync(log, { force: true }),
    calls: () => existsSync(log) ? readFileSync(log, 'utf8').trim().split('\n').filter(Boolean).map(JSON.parse) : [],
  };
}
