import { spec } from 'node:test/reporters';
import { Readable } from 'node:stream';

export default async function* testDurationReporter(source) {
  const reporter = Readable.from(source).pipe(spec());
  for await (const output of reporter) {
    yield formatReporterOutput(output);
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
