import { execFileSync } from 'node:child_process';
import path from 'node:path';

export const MAX_RELEASE_PATCH_BYTES = 160 * 1024;

const LOCK_FILE_NAMES = new Set([
  'Cargo.lock',
  'bun.lock',
  'bun.lockb',
  'package-lock.json',
  'pnpm-lock.yaml',
  'yarn.lock',
]);

const GENERATED_PATH_PREFIXES = [
  'dist/',
  'packaging/fnos/.stage/',
  'packaging/fnos/app/ui/dist/',
  'packaging/fnos/dist/',
  'server/target/',
];

const GENERATED_PATHS = new Set([
  'packaging/fnos/app/bin/aria2-next',
  'packaging/fnos/app/bin/motrix-fnos-server',
]);

export function readReleaseCommits(repoRoot, baseRef, headRef = 'HEAD') {
  const output = git(repoRoot, [
    'log',
    `${baseRef}..${headRef}`,
    '--no-merges',
    '--pretty=format:%h%x1f%s%x1f%b%x1e',
  ]);

  return output
    .split('\x1e')
    .map((record) => record.trim())
    .filter(Boolean)
    .map((record) => {
      const [hash = '', subject = '', ...bodyParts] = record.split('\x1f');
      return {
        hash: hash.trim(),
        subject: subject.trim(),
        body: bodyParts.join('\x1f').trim(),
      };
    });
}

export function collectReleaseChangeContext({
  repoRoot,
  baseRef,
  headRef = 'HEAD',
  commits = readReleaseCommits(repoRoot, baseRef, headRef),
  maxPatchBytes = MAX_RELEASE_PATCH_BYTES,
}) {
  const range = `${baseRef}..${headRef}`;
  const fileStatus = git(repoRoot, ['diff', '--no-renames', '--name-status', range]);
  const numstat = git(repoRoot, ['diff', '--no-renames', '--numstat', range]);
  const diffStat = git(repoRoot, ['diff', '--no-renames', '--stat', '--summary', range]);
  const changedPaths = readChangedPaths(repoRoot, range);
  const binaryPaths = readBinaryPaths(numstat);
  const omittedPatchFiles = [];
  const patchPaths = [];

  for (const changedPath of changedPaths) {
    const reason = omittedPatchReason(changedPath, binaryPaths);
    if (reason) {
      omittedPatchFiles.push({ path: changedPath, reason });
    } else {
      patchPaths.push(changedPath);
    }
  }

  const patch = patchPaths.length > 0
    ? git(repoRoot, [
        'diff',
        '--no-ext-diff',
        '--no-color',
        '--no-renames',
        '--unified=2',
        range,
        '--',
        ...patchPaths.map((changedPath) => `:(literal)${changedPath}`),
      ])
    : '';
  const patchBytes = Buffer.byteLength(patch, 'utf8');
  if (patchBytes > maxPatchBytes) {
    throw new Error(
      `发布文本补丁为 ${patchBytes} 字节，超过 ${maxPatchBytes} 字节上限；请人工预写目标版本 CHANGELOG 后重试`,
    );
  }

  const patchFiles = splitPatchFiles(patch, patchPaths);

  return {
    commits,
    fileStatus,
    numstat,
    diffStat,
    patch,
    patchFiles,
    patchBytes,
    omittedPatchFiles,
  };
}

function splitPatchFiles(patch, patchPaths) {
  if (!patch) return [];
  const sections = patch.split(/(?=^diff --git )/m).filter(Boolean);
  if (sections.length !== patchPaths.length) {
    return [{ path: '多个文本文件', patch }];
  }
  return sections.map((section, index) => ({ path: patchPaths[index], patch: section.trim() }));
}

function readChangedPaths(repoRoot, range) {
  const output = git(repoRoot, ['diff', '--no-renames', '--name-only', '-z', range], { trim: false });
  return output.split('\0').filter(Boolean);
}

function readBinaryPaths(numstat) {
  const paths = new Set();
  for (const line of numstat.split('\n')) {
    const [added, deleted, changedPath] = line.split('\t');
    if (changedPath && (added === '-' || deleted === '-')) {
      paths.add(changedPath);
    }
  }
  return paths;
}

function omittedPatchReason(changedPath, binaryPaths) {
  if (binaryPaths.has(changedPath)) return '二进制文件';
  if (LOCK_FILE_NAMES.has(path.posix.basename(changedPath))) return '锁文件';
  if (GENERATED_PATHS.has(changedPath)) return '构建产物';
  if (GENERATED_PATH_PREFIXES.some((prefix) => changedPath.startsWith(prefix))) return '生成目录';
  return null;
}

function git(repoRoot, args, { trim = true } = {}) {
  const output = execFileSync('git', args, {
    cwd: repoRoot,
    encoding: 'utf8',
    maxBuffer: 16 * 1024 * 1024,
  });
  return trim ? output.trim() : output;
}
