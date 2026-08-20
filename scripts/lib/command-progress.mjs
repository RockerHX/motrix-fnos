import { spawn } from 'node:child_process';
import { performance } from 'node:perf_hooks';
import process from 'node:process';
import { StringDecoder } from 'node:string_decoder';
import { stripVTControlCharacters } from 'node:util';

const SPINNER_FRAMES = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
const INTERACTIVE_DELAY_MS = 500;
const INTERACTIVE_INTERVAL_MS = 80;
const HEARTBEAT_INTERVAL_MS = 30_000;
const NESTED_PROGRESS_ENV = 'MOTRIX_FNOS_PROGRESS_PARENT';
const PROGRESS_MESSAGE_PREFIX = '\u001eMOTRIX_FNOS_PROGRESS ';

export function formatProgressLine(title, elapsedMilliseconds, frame = SPINNER_FRAMES[0]) {
  return `${frame} ${title} · 已运行 ${(elapsedMilliseconds / 1000).toFixed(2)}s`;
}

export function formatProgressDetailLine(detail, frame = SPINNER_FRAMES[1]) {
  return `${frame} ${detail || '正在启动子任务...'}`;
}

export function formatProgressMessage(detail) {
  return `${PROGRESS_MESSAGE_PREFIX}${JSON.stringify(String(detail))}\n`;
}

export function reportCommandProgress(detail, stream = process.stderr) {
  if (process.env[NESTED_PROGRESS_ENV] === '1') {
    stream.write(formatProgressMessage(detail));
  }
}

export function cargoProgressDetail(line) {
  const normalized = stripVTControlCharacters(String(line)).trim();
  const packageMatch = normalized.match(/^(Compiling|Checking|Downloading|Downloaded)\s+(.+)$/);
  if (packageMatch) {
    const actions = {
      Compiling: '正在编译',
      Checking: '正在检查',
      Downloading: '正在下载',
      Downloaded: '已下载',
    };
    return `${actions[packageMatch[1]]}：${packageMatch[2]}`;
  }
  const buildMatch = normalized.match(/^Building \[[^\]]*\]\s+(\d+\/\d+):\s*(.+)$/);
  if (buildMatch) {
    return `正在编译：${buildMatch[2]}（${buildMatch[1]}）`;
  }
  if (/^Finished\b/.test(normalized)) {
    return 'Cargo 编译已完成';
  }
  return null;
}

export function runCommandWithProgress(command, args, options) {
  const {
    title,
    cwd = process.cwd(),
    env = process.env,
    stdout = process.stdout,
    stderr = process.stderr,
    progressStream = process.stderr,
    initialDetail = '正在启动子任务...',
    activity,
    onProgress,
  } = options;
  const output = [];
  const progress = startProgress(title, progressStream, initialDetail);
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

  const stdoutRouter = createOutputRouter('stdout', stdout);
  const stderrRouter = createOutputRouter('stderr', stderr);
  child.stdout?.on('data', stdoutRouter.push);
  child.stderr?.on('data', stderrRouter.push);

  return new Promise((resolve, reject) => {
    let settled = false;
    let spawnError;
    child.on('error', (error) => {
      spawnError = error;
    });
    child.on('close', (code, signal) => {
      if (spawnError) {
        finish(() => reject(commandError(title, spawnError)));
        return;
      }
      if (code === 0) {
        finish(resolve);
        return;
      }
      finish(() => reject(commandError(title, signal ?? code)));
    });

    function finish(settle) {
      if (settled) return;
      settled = true;
      stdoutRouter.flush();
      stderrRouter.flush();
      progress.stop();
      for (const entry of output) {
        entry.stream.write(entry.chunk);
      }
      settle();
    }
  });

  function createOutputRouter(channel, stream) {
    const decoder = new StringDecoder('utf8');
    let pending = '';
    return {
      push(chunk) {
        pending += decoder.write(chunk);
        drainLines();
      },
      flush() {
        pending += decoder.end();
        if (pending) routeLine(pending);
        pending = '';
      },
    };

    function drainLines() {
      let newlineIndex;
      while ((newlineIndex = pending.indexOf('\n')) !== -1) {
        const line = pending.slice(0, newlineIndex + 1);
        pending = pending.slice(newlineIndex + 1);
        routeLine(line);
      }
    }

    function routeLine(line) {
      const reportedDetail = parseProgressMessage(line);
      if (reportedDetail !== null) {
        updateProgress(reportedDetail);
        return;
      }
      const detectedDetail = activity?.(line, channel);
      if (detectedDetail) {
        updateProgress(detectedDetail);
        return;
      }
      output.push({ stream, chunk: line });
    }
  }

  function updateProgress(detail) {
    progress.update(detail);
    onProgress?.(detail);
  }
}

function startProgress(title, stream, initialDetail) {
  if (process.env[NESTED_PROGRESS_ENV] === '1') {
    reportCommandProgress(initialDetail);
    return {
      update(detail) {
        reportCommandProgress(detail);
      },
      stop() {},
    };
  }

  const startedAt = performance.now();
  let detail = initialDetail;
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
      stream.write(`[进行中] ${title} · 已运行 ${((performance.now() - startedAt) / 1000).toFixed(2)}s · ${detail}\n`);
    }, HEARTBEAT_INTERVAL_MS);
    interval.unref();
  }

  const clearOnExit = () => clearInteractiveLine();
  process.once('exit', clearOnExit);

  return {
    update(nextDetail) {
      detail = nextDetail;
    },
    stop() {
      clearTimeout(delay);
      clearInterval(interval);
      process.removeListener('exit', clearOnExit);
      clearInteractiveLine();
    },
  };

  function render() {
    const columns = Number.isInteger(stream.columns) ? Math.max(stream.columns - 1, 20) : 120;
    if (rendered) {
      clearInteractiveLines();
    }
    rendered = true;
    const titleLine = fitProgressLine(
      formatProgressLine(title, performance.now() - startedAt, SPINNER_FRAMES[frameIndex]),
      columns,
    );
    const detailLine = fitProgressLine(
      formatProgressDetailLine(detail, SPINNER_FRAMES[(frameIndex + 1) % SPINNER_FRAMES.length]),
      columns,
    );
    stream.write(`${titleLine}\n\r${detailLine}`);
    frameIndex = (frameIndex + 1) % SPINNER_FRAMES.length;
  }

  function clearInteractiveLine() {
    if (rendered) {
      clearInteractiveLines();
      rendered = false;
    }
  }

  function clearInteractiveLines() {
    stream.write('\r\u001b[2K\u001b[1A\r\u001b[2K');
  }
}

export function fitProgressLine(value, maximumWidth) {
  const text = String(value);
  if (displayWidth(text) <= maximumWidth) return text;
  const suffix = '...';
  const targetWidth = Math.max(0, maximumWidth - suffix.length);
  let result = '';
  let width = 0;
  for (const character of text) {
    const nextWidth = characterWidth(character);
    if (width + nextWidth > targetWidth) break;
    result += character;
    width += nextWidth;
  }
  return `${result}${suffix}`;
}

function parseProgressMessage(line) {
  const normalized = String(line).replace(/\r?\n$/, '');
  if (!normalized.startsWith(PROGRESS_MESSAGE_PREFIX)) return null;
  try {
    const detail = JSON.parse(normalized.slice(PROGRESS_MESSAGE_PREFIX.length));
    return typeof detail === 'string' ? detail : null;
  } catch {
    return null;
  }
}

function displayWidth(value) {
  return [...value].reduce((width, character) => width + characterWidth(character), 0);
}

function characterWidth(character) {
  const codePoint = character.codePointAt(0) ?? 0;
  return codePoint > 0xff && !(codePoint >= 0x2800 && codePoint <= 0x28ff) ? 2 : 1;
}

function commandError(title, reason) {
  const detail = reason instanceof Error ? reason.message : String(reason);
  const error = new Error(`${title}失败：${detail}`);
  if (Number.isInteger(reason)) {
    error.exitCode = reason;
  }
  return error;
}
