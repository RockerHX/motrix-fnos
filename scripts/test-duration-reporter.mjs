import { spec } from 'node:test/reporters';
import { Readable } from 'node:stream';
import { stripVTControlCharacters } from 'node:util';

export default async function* testDurationReporter(source) {
  const reporter = Readable.from(source).pipe(spec());
  const summary = {};
  for await (const output of reporter) {
    const formattedOutput = formatReporterOutput(output);
    const summaryEntry = parseReporterSummary(formattedOutput);
    if (summaryEntry) {
      summary[summaryEntry.key] = summaryEntry.value;
    } else if (!isPassedTestOutput(formattedOutput)) {
      yield formattedOutput;
    }
  }
  const summaryOutput = formatReporterSummary(summary);
  if (summaryOutput) {
    yield summaryOutput;
  }
}

export function formatDuration(milliseconds) {
  return milliseconds >= 1000
    ? `${(milliseconds / 1000).toFixed(2)}s`
    : `${milliseconds.toFixed(2)}ms`;
}

export function formatReporterOutput(output) {
  return String(output)
    .replace(/\((\d+(?:\.\d+)?)ms\)/g, (_match, value) => `(${formatDuration(Number(value))})`)
    .replace(/\bduration_ms (\d+(?:\.\d+)?)/g, (_match, value) => `duration ${formatDuration(Number(value))}`);
}

export function isPassedTestOutput(output) {
  return stripVTControlCharacters(String(output)).trimStart().startsWith('✔ ');
}

export function parseReporterSummary(output) {
  const normalized = stripVTControlCharacters(String(output)).trim();
  const countMatch = normalized.match(/^ℹ (tests|suites|pass|fail|cancelled|skipped|todo) (\d+)$/);
  if (countMatch) {
    return { key: countMatch[1], value: Number(countMatch[2]) };
  }
  const durationMatch = normalized.match(/^ℹ duration (.+)$/);
  return durationMatch ? { key: 'duration', value: durationMatch[1] } : null;
}

export function formatReporterSummary(summary) {
  if (!Number.isInteger(summary.tests)) {
    return '';
  }
  const results = [`${summary.pass ?? 0} passed`];
  for (const key of ['fail', 'cancelled', 'skipped', 'todo']) {
    if ((summary[key] ?? 0) > 0) {
      results.push(`${summary[key]} ${key}`);
    }
  }
  return `ℹ ${summary.tests} tests: ${results.join(', ')}${summary.duration ? ` (${summary.duration})` : ''}\n`;
}
