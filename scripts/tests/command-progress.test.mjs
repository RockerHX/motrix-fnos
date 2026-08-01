import assert from 'node:assert/strict';
import { Writable } from 'node:stream';
import test from 'node:test';
import { formatProgressLine, runCommandWithProgress } from '../command-progress.mjs';

test('长任务进度使用单行阶段名称和两位小数秒数', () => {
  assert.equal(formatProgressLine('Rust 测试', 1234.56, '|'), '| Rust 测试 · 已运行 1.23s');
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
