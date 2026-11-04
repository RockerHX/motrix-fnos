import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';

const hook = path.resolve(import.meta.dirname, '../../.githooks/pre-push');
const zero = '0'.repeat(40);
const verify = [['pnpm', 'run', 'verify', '--', '--reuse']];
const changelog = [
  ['node', '--test', 'scripts/tests/release-notes.test.mjs'],
  ['pnpm', 'run', 'test:unit', 'src/features/about/services/changelogService.spec.ts'],
  ['pnpm', 'run', 'build'],
];

for (const [title, changes, expected] of [
  ['中文文档和说明图片跳过', { 'docs/中文说明.md': '说明', 'docs/logo.svg': '<svg/>' }, []],
  ['网页图片需要核对前端验证', { 'public/logo.svg': '<svg/>' }, verify],
  ['CHANGELOG 只校验发布说明、解析和前端构建', { 'CHANGELOG.md': '更新' }, changelog],
  ['CHANGELOG 和普通文档仍走定向验证', { 'CHANGELOG.md': '更新', 'README.md': 'docs' }, changelog],
  ['CHANGELOG 和维护文档分别执行必要检查', { 'CHANGELOG.md': '更新', 'docs/maintenance.md': 'docs' },
    [['node', '--test', 'scripts/tests/logging-docs.test.mjs'], ...changelog]],
  ['CHANGELOG 和前端测试不重复运行解析测试', { 'CHANGELOG.md': '更新', 'src/a.spec.ts': 'test' },
    [changelog[0], ['pnpm', 'run', 'test:unit'], changelog[2]]],
  ['CHANGELOG 和脚本测试不重复校验发布说明', { 'CHANGELOG.md': '更新', 'scripts/tests/a.test.mjs': 'test' },
    [['pnpm', 'run', 'test:scripts'], ...changelog.slice(1)]],
  ['CHANGELOG 混有源码仍完整验证', { 'CHANGELOG.md': '更新', 'src/app.ts': 'changed' }, verify],
  ['源码目录内文本不能误跳过', { 'src/example.txt': 'runtime input' }, verify],
  ['Rust 夹具内文本仍执行 Rust 测试', { 'server/tests/example.txt': 'fixture' }, [['node', 'scripts/verify/run-rust-tests.mjs']]],
  ['打包图标不能误跳过', { 'packaging/fnos/ICON.PNG': 'image' }, verify],
  ['Aria2 资产不能误跳过', { 'assets/aria2/aria2-next-linux': 'binary' }, verify],
  ['维护文档执行已有文档断言', { 'docs/maintenance.md': 'documentation' }, [['node', '--test', 'scripts/tests/logging-docs.test.mjs']]],
  ['文档断言已包含于脚本测试时不重复执行', { 'docs/api-contract.md': 'documentation', 'scripts/tests/example.test.mjs': 'test' }, [['pnpm', 'run', 'test:scripts']]],
  ['纯文档删除跳过', { 'README.md': null }, []],
  ['前端测试只跑单元测试', { 'src/example.spec.ts': 'test' }, [['pnpm', 'run', 'test:unit']]],
  ['前端共享夹具只跑单元测试', { 'src/test/tasks.ts': 'fixture' }, [['pnpm', 'run', 'test:unit']]],
  ['Rust 模块测试只跑 Rust 测试', { 'server/src/tasks/tests.rs': 'test' }, [['node', 'scripts/verify/run-rust-tests.mjs']]],
  ['Rust 集成测试夹具只跑 Rust 测试', { 'server/tests/fixtures/task.json': '{}' }, [['node', 'scripts/verify/run-rust-tests.mjs']]],
  ['Rust 共享工具只跑 Rust 测试', { 'server/src/test_support.rs': 'fixture' }, [['node', 'scripts/verify/run-rust-tests.mjs']]],
  ['稳定性夹具同时验证日常与完整专项', { 'server/tests/runtime_stability/resources.rs': 'test' },
    [['node', 'scripts/verify/run-rust-tests.mjs'], ['pnpm', 'run', 'test:stability']]],
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

test('pre-push：CHANGELOG 各步失败立即阻止推送，npm 降级仍只运行定向检查', (t) => {
  for (let failAt = 1; failAt <= changelog.length; failAt += 1) {
    const fixture = repository(t);
    fixture.commit({ 'CHANGELOG.md': '更新' });
    assert.equal(fixture.push({ env: { PUSH_FAIL_AT: String(failAt) } }).status, 19);
    assert.deepEqual(fixture.calls(), changelog.slice(0, failAt));
  }
  const fixture = repository(t);
  fixture.commit({ 'CHANGELOG.md': '更新' });
  assert.equal(fixture.push({ env: { PUSH_NO_PNPM: '1' } }).status, 0);
  assert.deepEqual(fixture.calls(), [changelog[0],
    ['npm', 'run', 'test:unit', '--', 'src/features/about/services/changelogService.spec.ts'],
    ['npm', 'run', 'build']]);
});

test('pre-push：删除或移走 CHANGELOG 仍执行解析和构建检查', (t) => {
  for (const renamed of [false, true]) {
    const fixture = repository(t);
    const remote = fixture.commit({ 'CHANGELOG.md': '更新' });
    fixture.commit({ 'CHANGELOG.md': null, ...(renamed ? { 'docs/history.md': '更新' } : {}) });
    assert.equal(fixture.push({ remote }).status, 0);
    assert.deepEqual(fixture.calls(), changelog);
  }
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
