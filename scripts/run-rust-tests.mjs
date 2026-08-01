#!/usr/bin/env node
import { spawnSync } from 'node:child_process';
import process from 'node:process';

export function summarizeRustTestOutput(output) {
  const summaries = [...String(output).matchAll(
    /test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out;/g,
  )];
  if (summaries.length === 0) {
    return null;
  }
  return summaries.reduce(
    (total, match) => ({
      targets: total.targets + 1,
      passed: total.passed + Number(match[1]),
      failed: total.failed + Number(match[2]),
      ignored: total.ignored + Number(match[3]),
      measured: total.measured + Number(match[4]),
      filteredOut: total.filteredOut + Number(match[5]),
    }),
    { targets: 0, passed: 0, failed: 0, ignored: 0, measured: 0, filteredOut: 0 },
  );
}

export function formatRustTestSummary(summary) {
  const results = [`${summary.passed} passed`];
  if (summary.ignored > 0) {
    results.push(`${summary.ignored} ignored`);
  }
  return `Rust 测试汇总：${results.join('，')}，${summary.targets} 个测试目标。`;
}

if (process.argv[1]?.endsWith('run-rust-tests.mjs')) {
  const result = spawnSync(
    'cargo',
    ['test', '--quiet', '--manifest-path', 'server/Cargo.toml'],
    {
      cwd: process.cwd(),
      env: process.env,
      encoding: 'utf8',
      maxBuffer: 64 * 1024 * 1024,
      stdio: ['inherit', 'pipe', 'inherit'],
    },
  );

  if (result.error) {
    console.error(`Rust 测试启动失败：${result.error.message}`);
    process.exit(1);
  }
  if (result.status !== 0) {
    process.stdout.write(result.stdout ?? '');
    process.exit(result.status ?? 1);
  }

  const summary = summarizeRustTestOutput(result.stdout);
  if (!summary || summary.failed > 0) {
    process.stdout.write(result.stdout ?? '');
    console.error('Rust 测试成功退出，但无法生成可信汇总。');
    process.exit(1);
  }

  console.log(formatRustTestSummary(summary));
}
