import assert from 'node:assert/strict';
import test from 'node:test';
import { formatRustTestSummary, summarizeRustTestOutput } from '../run-rust-tests.mjs';

test('Rust 测试输出汇总多个测试目标', () => {
  const output = [
    'test result: ok. 333 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 42.29s',
    'test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s',
    'test result: ok. 3 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 1.64s',
    'test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.97s',
  ].join('\n');

  assert.deepEqual(summarizeRustTestOutput(output), {
    targets: 4,
    passed: 337,
    failed: 0,
    ignored: 1,
    measured: 0,
    filteredOut: 0,
  });
});

test('Rust 测试输出缺少原生结果时拒绝生成汇总', () => {
  assert.equal(summarizeRustTestOutput('cargo exited without test results'), null);
});

test('Rust 测试汇总保留被忽略的测试数量', () => {
  assert.equal(
    formatRustTestSummary({ passed: 337, ignored: 1, targets: 5 }),
    'Rust 测试汇总：337 passed，1 ignored，5 个测试目标。',
  );
});
