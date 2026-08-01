import { spawn } from 'node:child_process';
import { performance } from 'node:perf_hooks';
import process from 'node:process';

const SPINNER_FRAMES = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
const INTERACTIVE_DELAY_MS = 500;
const INTERACTIVE_INTERVAL_MS = 80;
const HEARTBEAT_INTERVAL_MS = 30_000;
const NESTED_PROGRESS_ENV = 'MOTRIX_FNOS_PROGRESS_PARENT';

export function formatProgressLine(title, elapsedMilliseconds, frame = SPINNER_FRAMES[0]) {
  return `${frame} ${title} · 已运行 ${(elapsedMilliseconds / 1000).toFixed(2)}s`;
}

export function runCommandWithProgress(command, args, options) {
  const {
    title,
    cwd = process.cwd(),
    env = process.env,
    stdout = process.stdout,
    stderr = process.stderr,
    progressStream = process.stderr,
  } = options;
  const output = [];
  const progress = startProgress(title, progressStream);
  let child;

  try {
    child = spawn(command, args, {
      cwd,
      env: { ...env, [NESTED_PROGRESS_ENV]: '1' },
      stdio: ['inherit', 'pipe', 'pipe'],
      shell: false,
    });
  } catch (error) {
    progress.stop();
    return Promise.reject(commandError(title, error));
  }

  child.stdout?.on('data', (chunk) => output.push({ stream: stdout, chunk }));
  child.stderr?.on('data', (chunk) => output.push({ stream: stderr, chunk }));

  return new Promise((resolve, reject) => {
    let settled = false;
    child.on('error', (error) => finish(() => reject(commandError(title, error))));
    child.on('close', (code, signal) => {
      if (code === 0) {
        finish(resolve);
        return;
      }
      finish(() => reject(commandError(title, signal ?? code)));
    });

    function finish(settle) {
      if (settled) return;
      settled = true;
      progress.stop();
      for (const entry of output) {
        entry.stream.write(entry.chunk);
      }
      settle();
    }
  });
}

function startProgress(title, stream) {
  if (process.env[NESTED_PROGRESS_ENV] === '1') {
    return { stop() {} };
  }

  const startedAt = performance.now();
  let frameIndex = 0;
  let rendered = false;
  let interval;
  let delay;

  if (stream.isTTY) {
    delay = setTimeout(() => {
      render();
      interval = setInterval(render, INTERACTIVE_INTERVAL_MS);
      interval.unref();
    }, INTERACTIVE_DELAY_MS);
    delay.unref();
  } else {
    interval = setInterval(() => {
      stream.write(`[进行中] ${title} · 已运行 ${((performance.now() - startedAt) / 1000).toFixed(2)}s\n`);
    }, HEARTBEAT_INTERVAL_MS);
    interval.unref();
  }

  const clearOnExit = () => clearInteractiveLine();
  process.once('exit', clearOnExit);

  return {
    stop() {
      clearTimeout(delay);
      clearInterval(interval);
      process.removeListener('exit', clearOnExit);
      clearInteractiveLine();
    },
  };

  function render() {
    rendered = true;
    stream.write(`\r\u001b[2K${formatProgressLine(title, performance.now() - startedAt, SPINNER_FRAMES[frameIndex])}`);
    frameIndex = (frameIndex + 1) % SPINNER_FRAMES.length;
  }

  function clearInteractiveLine() {
    if (rendered) {
      stream.write('\r\u001b[2K');
      rendered = false;
    }
  }
}

function commandError(title, reason) {
  const detail = reason instanceof Error ? reason.message : String(reason);
  const error = new Error(`${title}失败：${detail}`);
  if (Number.isInteger(reason)) {
    error.exitCode = reason;
  }
  return error;
}
