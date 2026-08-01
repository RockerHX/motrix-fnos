import assert from 'node:assert/strict';
import test from 'node:test';
import {
  formatDuration,
  formatReporterOutput,
  formatReporterSummary,
  isPassedTestOutput,
  parseReporterSummary,
} from '../test-duration-reporter.mjs';

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

test('测试 reporter 隐藏成功项并保留失败、跳过和汇总信息', () => {
  assert.equal(isPassedTestOutput('✔ 成功测试 (1.00ms)\n'), true);
  assert.equal(isPassedTestOutput('\u001b[32m✔ 成功测试 (1.00ms)\u001b[39m\n'), true);
  assert.equal(isPassedTestOutput('✖ 失败测试 (1.00ms)\n'), false);
  assert.equal(isPassedTestOutput('﹣ 跳过测试\n'), false);
  assert.equal(isPassedTestOutput('ℹ tests 3\n'), false);
});

test('测试 reporter 将多行统计合并为单行摘要', () => {
  assert.deepEqual(parseReporterSummary('ℹ tests 45\n'), { key: 'tests', value: 45 });
  assert.deepEqual(parseReporterSummary('ℹ duration 16.08s\n'), { key: 'duration', value: '16.08s' });
  assert.equal(parseReporterSummary('✖ 失败测试\n'), null);
  assert.equal(
    formatReporterSummary({ tests: 45, pass: 43, fail: 1, skipped: 1, duration: '16.08s' }),
    'ℹ 45 tests: 43 passed, 1 fail, 1 skipped (16.08s)\n',
  );
});
