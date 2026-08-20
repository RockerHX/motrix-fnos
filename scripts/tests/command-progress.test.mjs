import assert from 'node:assert/strict';
import { Writable } from 'node:stream';
import test from 'node:test';
import {
  cargoProgressDetail,
  fitProgressLine,
  formatProgressDetailLine,
  formatProgressLine,
  formatProgressMessage,
  runCommandWithProgress,
} from '../lib/command-progress.mjs';

test('长任务进度使用单行阶段名称和两位小数秒数', () => {
  assert.equal(formatProgressLine('Rust 测试', 1234.56, '|'), '| Rust 测试 · 已运行 1.23s');
});

test('第二行进度展示当前子任务并限制终端宽度', () => {
  assert.equal(formatProgressDetailLine('正在测试：EngineStatusPanel.spec.ts', '/'), '/ 正在测试：EngineStatusPanel.spec.ts');
  assert.equal(fitProgressLine('1234567890', 8), '12345...');
});

test('Cargo 成功明细转换为当前编译任务', () => {
  assert.equal(cargoProgressDetail('   Compiling motrix-fnos-server v1.8.4'), '正在编译：motrix-fnos-server v1.8.4');
  assert.equal(cargoProgressDetail('warning: unused value'), null);
});

test('长任务成功后回放摘要输出', async () => {
  const output = memoryStream();
  const errors = memoryStream();
  await runCommandWithProgress(process.execPath, ['-e', "console.log('2 tests passed')"], {
    title: '测试任务',
    stdout: output.stream,
    stderr: errors.stream,
    progressStream: errors.stream,
  });

  assert.equal(output.read(), '2 tests passed\n');
  assert.equal(errors.read(), '');
});

test('子进度消息更新第二行但不进入最终日志', async () => {
  const output = memoryStream();
  const errors = memoryStream();
  const progress = [];
  const message = formatProgressMessage('正在测试：TaskList.spec.ts');
  await runCommandWithProgress(
    process.execPath,
    ['-e', `process.stderr.write(${JSON.stringify(message)}); console.log('done')`],
    {
      title: '测试任务',
      stdout: output.stream,
      stderr: errors.stream,
      progressStream: errors.stream,
      onProgress: (detail) => progress.push(detail),
    },
  );

  assert.deepEqual(progress, ['正在测试：TaskList.spec.ts']);
  assert.equal(output.read(), 'done\n');
  assert.equal(errors.read(), '');
});

test('长任务失败时回放诊断并保留退出码', async () => {
  const output = memoryStream();
  const errors = memoryStream();
  await assert.rejects(
    runCommandWithProgress(process.execPath, ['-e', "console.error('build failed'); process.exit(7)"], {
      title: '失败任务',
      stdout: output.stream,
      stderr: errors.stream,
      progressStream: errors.stream,
    }),
    (error) => error.message === '失败任务失败：7' && error.exitCode === 7,
  );

  assert.equal(errors.read(), 'build failed\n');
});

function memoryStream() {
  const chunks = [];
  return {
    stream: new Writable({
      write(chunk, _encoding, callback) {
        chunks.push(Buffer.from(chunk));
        callback();
      },
    }),
    read: () => Buffer.concat(chunks).toString('utf8'),
  };
}
