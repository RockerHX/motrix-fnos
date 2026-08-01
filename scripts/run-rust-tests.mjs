#!/usr/bin/env node
import { spawn } from 'node:child_process';
import process from 'node:process';
import { StringDecoder } from 'node:string_decoder';
import { cargoProgressDetail, reportCommandProgress } from './command-progress.mjs';

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

export function rustTestProgressDetail(line) {
  const normalized = String(line).trim();
  const countMatch = normalized.match(/^running (\d+) tests?$/);
  if (countMatch && Number(countMatch[1]) > 0) return `正在运行：${countMatch[1]} 项 Rust 测试`;
  const testMatch = normalized.match(/^test (.+) \.\.\. (ok|ignored|FAILED)$/);
  if (!testMatch) return null;
  return testMatch[2] === 'FAILED'
    ? `测试失败：${testMatch[1]}`
    : `最近完成：${testMatch[1]}`;
}

export function rustCargoProgressDetail(line) {
  const cargoDetail = cargoProgressDetail(line);
  if (cargoDetail) return cargoDetail;
  const normalized = String(line).trim();
  const runningMatch = normalized.match(/^Running\s+(.+)$/);
  if (runningMatch) return `启动测试目标：${runningMatch[1]}`;
  const docTestMatch = normalized.match(/^Doc-tests\s+(.+)$/);
  return docTestMatch ? `启动文档测试目标：${docTestMatch[1]}` : null;
}

if (process.argv[1]?.endsWith('run-rust-tests.mjs')) {
  await runRustTests();
}

async function runRustTests() {
  const child = spawn('cargo', ['test', '--manifest-path', 'server/Cargo.toml'], {
    cwd: process.cwd(),
    env: process.env,
    stdio: ['inherit', 'pipe', 'pipe'],
  });
  const stdoutChunks = [];
  const stdoutLines = observeLines((line) => {
    const detail = rustTestProgressDetail(line);
    if (detail) reportCommandProgress(detail);
  });
  const stderr = filterLines((line) => {
    const detail = rustCargoProgressDetail(line);
    if (detail) reportCommandProgress(detail);
    return !detail;
  });

  child.stdout.on('data', (chunk) => {
    stdoutChunks.push(Buffer.from(chunk));
    stdoutLines.push(chunk);
  });
  child.stderr.on('data', stderr.push);

  let spawnError;
  child.on('error', (error) => {
    spawnError = error;
  });
  const { code } = await new Promise((resolve) => {
    child.on('close', (code) => resolve({ code }));
  });
  stdoutLines.flush();
  stderr.flush();

  const stdout = Buffer.concat(stdoutChunks).toString('utf8');
  const stderrOutput = stderr.output();
  if (spawnError) {
    console.error(`Rust 测试启动失败：${spawnError.message}`);
    process.exit(1);
  }
  if (code !== 0) {
    process.stdout.write(stdout);
    process.stderr.write(stderrOutput);
    process.exit(code ?? 1);
  }

  const summary = summarizeRustTestOutput(stdout);
  if (!summary || summary.failed > 0) {
    process.stdout.write(stdout);
    process.stderr.write(stderrOutput);
    console.error('Rust 测试成功退出，但无法生成可信汇总。');
    process.exit(1);
  }

  process.stderr.write(stderrOutput);
  console.log(formatRustTestSummary(summary));
}

function observeLines(onLine) {
  const decoder = new StringDecoder('utf8');
  let pending = '';
  return {
    push(chunk) {
      pending += decoder.write(chunk);
      drain();
    },
    flush() {
      pending += decoder.end();
      if (pending) onLine(pending);
    },
  };

  function drain() {
    let newlineIndex;
    while ((newlineIndex = pending.indexOf('\n')) !== -1) {
      onLine(pending.slice(0, newlineIndex));
      pending = pending.slice(newlineIndex + 1);
    }
  }
}

function filterLines(keepLine) {
  const decoder = new StringDecoder('utf8');
  const output = [];
  let pending = '';
  return {
    push(chunk) {
      pending += decoder.write(chunk);
      drain();
    },
    flush() {
      pending += decoder.end();
      if (pending) route(pending);
      pending = '';
    },
    output: () => output.join(''),
  };

  function drain() {
    let newlineIndex;
    while ((newlineIndex = pending.indexOf('\n')) !== -1) {
      const line = pending.slice(0, newlineIndex + 1);
      pending = pending.slice(newlineIndex + 1);
      route(line);
    }
  }

  function route(line) {
    if (keepLine(line)) output.push(line);
  }
}
