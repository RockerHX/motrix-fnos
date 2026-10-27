import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';

const hook = path.resolve(import.meta.dirname, '../../.githooks/pre-push');
const zero = '0'.repeat(40);
const verify = [['pnpm', 'run', 'verify', '--', '--reuse']];

for (const [title, changes, expected] of [
  ['中文文档和图片跳过', { 'docs/中文说明.md': '说明', 'public/logo.svg': '<svg/>' }, []],
  ['纯文档删除跳过', { 'README.md': null }, []],
  ['前端测试只跑单元测试', { 'src/example.spec.ts': 'test' }, [['pnpm', 'run', 'test:unit']]],
  ['前端共享夹具只跑单元测试', { 'src/test/tasks.ts': 'fixture' }, [['pnpm', 'run', 'test:unit']]],
  ['Rust 模块测试只跑 Rust 测试', { 'server/src/tasks/tests.rs': 'test' }, [['node', 'scripts/verify/run-rust-tests.mjs']]],
  ['Rust 集成测试夹具只跑 Rust 测试', { 'server/tests/fixtures/task.json': '{}' }, [['node', 'scripts/verify/run-rust-tests.mjs']]],
  ['Rust 共享工具只跑 Rust 测试', { 'server/src/test_support.rs': 'fixture' }, [['node', 'scripts/verify/run-rust-tests.mjs']]],
  ['脚本测试资源仍运行脚本测试', { 'scripts/tests/fixtures/readme.txt': 'fixture' }, [['pnpm', 'run', 'test:scripts']]],
  ['浏览器测试只跑 E2E', { 'e2e/browser.spec.ts': 'test' }, [['pnpm', 'run', 'test:e2e']]],
  ['引擎测试只跑 sidecar', { 'scripts/extended/aria2-sidecar.test.mjs': 'test' }, [['pnpm', 'run', 'test:aria2']]],
  ['Shell 测试直接运行', { 'scripts/verify/test-fnos-readiness.sh': 'test' }, [['sh', 'scripts/verify/test-fnos-readiness.sh']]],
  ['业务代码保持完整验证', { 'src/app.ts': 'changed' }, verify],
  ['删除业务代码保持完整验证', { 'src/app.ts': null }, verify],
  ['依赖保持完整验证', { 'package.json': '{}' }, verify],
  ['构建脚本保持完整验证', { 'scripts/build/build-fpk.mjs': 'changed' }, verify],
  ['文档目录中的代码不能误跳过', { 'docs/check.mjs': 'code' }, verify],
  ['删除保护范围前的重命名仍需验证', { 'src/app.ts': null, 'docs/app.md': 'code' }, verify],
  ['测试和文档混合按测试类别运行一次', { 'src/a.spec.ts': 'test', 'src/b.spec.ts': 'test', 'README.md': 'docs' }, [['pnpm', 'run', 'test:unit']]],
  ['同时修改测试和源码只跑一次完整验证', { 'src/a.spec.ts': 'test', 'src/app.ts': 'changed' }, verify],
]) {
  test(`pre-push：${title}`, (t) => {
    const fixture = repository(t);
    fixture.commit(changes);
    const result = fixture.push();
    assert.equal(result.status, 0, result.stderr);
    assert.deepEqual(fixture.calls(), expected);
  });
}

test('pre-push：检查全部待推送提交，不能因最后一个提交只有文档而漏测', (t) => {
  const fixture = repository(t);
  fixture.commit({ 'src/app.ts': 'changed' });
  fixture.commit({ 'README.md': 'docs' });
  assert.equal(fixture.push().status, 0);
  assert.deepEqual(fixture.calls(), verify);
});

test('pre-push：按实际推送的提交判断，不混入当前 HEAD 后续改动', (t) => {
  const fixture = repository(t);
  const docs = fixture.commit({ 'README.md': 'docs' });
  fixture.commit({ 'src/app.ts': 'changed' });
  assert.equal(fixture.push({ local: docs }).status, 0);
  assert.deepEqual(fixture.calls(), []);
});

test('pre-push：合并多个分支的范围，完整验证仅运行一次', (t) => {
  const fixture = repository(t);
  const docs = fixture.commit({ 'README.md': 'docs' });
  const code = fixture.commit({ 'src/app.ts': 'changed' });
  const input = `${fixture.ref(docs)}${fixture.ref(code, fixture.base, 'refs/heads/other')}`;
  assert.equal(fixture.push({ input }).status, 0);
  assert.deepEqual(fixture.calls(), verify);
});

test('pre-push：测试类别去重，失败后不继续执行后续测试', (t) => {
  const fixture = repository(t);
  fixture.commit({ 'scripts/tests/a.test.mjs': 'test', 'src/a.spec.ts': 'test' });
  const result = fixture.push({ env: { PUSH_FAIL_AT: '1' } });
  assert.equal(result.status, 19);
  assert.deepEqual(fixture.calls(), [['pnpm', 'run', 'test:scripts']]);
});

test('pre-push：删除测试文件仍运行所属测试类别', (t) => {
  const fixture = repository(t);
  const remote = fixture.commit({ 'src/a.spec.ts': 'test' });
  fixture.commit({ 'src/a.spec.ts': null });
  assert.equal(fixture.push({ remote }).status, 0);
  assert.deepEqual(fixture.calls(), [['pnpm', 'run', 'test:unit']]);
});

for (const remote of [zero, 'f'.repeat(40)]) {
  test(`pre-push：${remote === zero ? '新分支' : '远端对象不可读'}保守执行完整验证`, (t) => {
    const fixture = repository(t);
    fixture.commit({ 'README.md': 'docs' });
    assert.equal(fixture.push({ remote }).status, 0);
    assert.deepEqual(fixture.calls(), verify);
  });
}

for (const kind of ['tag', 'delete', 'empty', 'unchanged']) {
  test(`pre-push：${kind} 不触发测试`, (t) => {
    const fixture = repository(t);
    const input = kind === 'tag' ? fixture.ref(fixture.base, zero, 'refs/tags/v2.0.1')
      : kind === 'delete' ? fixture.ref(zero)
        : kind === 'empty' ? '' : fixture.ref(fixture.base);
    assert.equal(fixture.push({ input }).status, 0);
    assert.deepEqual(fixture.calls(), []);
  });
}

test('pre-push：pnpm 不可用时使用 npm，完整验证失败阻止推送', (t) => {
  const fixture = repository(t);
  fixture.commit({ 'src/app.ts': 'changed' });
  assert.equal(fixture.push({ env: { PUSH_NO_PNPM: '1', PUSH_FAIL_AT: '1' } }).status, 19);
  assert.deepEqual(fixture.calls(), [['npm', 'run', 'verify', '--', '--reuse']]);
});

function repository(t) {
  const root = mkdtempSync(path.join(os.tmpdir(), 'motrix-pre-push-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const git = (...args) => {
    const result = spawnSync('git', args, { cwd: root, encoding: 'utf8' });
    assert.equal(result.status, 0, result.stderr);
    return result.stdout.trim();
  };
  git('init', '--quiet');
  git('config', 'user.name', 'Hook Test');
  git('config', 'user.email', 'hook@example.test');
  git('config', 'core.hooksPath', '/dev/null');
  const commit = (changes) => {
    for (const [file, value] of Object.entries(changes)) {
      const target = path.join(root, file);
      if (value === null) rmSync(target);
      else {
        mkdirSync(path.dirname(target), { recursive: true });
        writeFileSync(target, value);
      }
    }
    git('add', '--all');
    git('commit', '--quiet', '--no-gpg-sign', '-m', 'fixture');
    return git('rev-parse', 'HEAD');
  };
  const base = commit({ 'src/app.ts': 'code', 'README.md': 'base documentation' });
  const bin = path.join(root, '.git', 'test-bin');
  const log = path.join(root, '.git', 'calls.jsonl');
  mkdirSync(bin);
  const stub = `#!${process.execPath}
const fs = require('node:fs');
const path = require('node:path');
const call = [path.basename(process.argv[1]), ...process.argv.slice(2)];
if (call[1] === '--version') process.exit(process.env.PUSH_NO_PNPM ? 127 : 0);
fs.appendFileSync(process.env.PUSH_LOG, JSON.stringify(call) + '\\n');
const count = fs.readFileSync(process.env.PUSH_LOG, 'utf8').trim().split('\\n').length;
if (count === Number(process.env.PUSH_FAIL_AT)) process.exit(19);
`;
  for (const command of ['pnpm', 'npm', 'node', 'sh']) writeFileSync(path.join(bin, command), stub, { mode: 0o755 });
  const ref = (local, remote = base, remoteRef = 'refs/heads/feature') => `refs/heads/feature ${local} ${remoteRef} ${remote}\n`;
  return {
    base, commit, ref,
    push: ({ remote = base, local = git('rev-parse', 'HEAD'), input = ref(local, remote), env = {} } = {}) =>
      spawnSync(process.execPath, [hook], {
        cwd: root, input, encoding: 'utf8', timeout: 10_000,
        env: { ...process.env, PATH: `${bin}${path.delimiter}${process.env.PATH ?? ''}`, PUSH_LOG: log, ...env },
      }),
    calls: () => existsSync(log) ? readFileSync(log, 'utf8').trim().split('\n').filter(Boolean).map((line) => JSON.parse(line)) : [],
  };
}
