import assert from 'node:assert/strict';
import test from 'node:test';
import { formatDuration, formatReporterOutput } from '../test-duration-reporter.mjs';

test('测试耗时按阈值使用两位小数的毫秒或秒', () => {
  assert.equal(formatDuration(0.199041), '0.20ms');
  assert.equal(formatDuration(7.641833), '7.64ms');
  assert.equal(formatDuration(997.234458), '997.23ms');
  assert.equal(formatDuration(1000), '1.00s');
  assert.equal(formatDuration(10018.575625), '10.02s');
});

test('测试 reporter 同时格式化单项和汇总耗时', () => {
  const output = [
    '✔ 快速测试 (7.641833ms)',
    '✔ 慢速测试 (10018.575625ms)',
    'ℹ duration_ms 11590.769458',
  ].join('\n');

  assert.equal(
    formatReporterOutput(output),
    [
      '✔ 快速测试 (7.64ms)',
      '✔ 慢速测试 (10.02s)',
      'ℹ duration 11.59s',
    ].join('\n'),
  );
});
