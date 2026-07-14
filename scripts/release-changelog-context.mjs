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

  return {
    commits,
    fileStatus,
    numstat,
    diffStat,
    patch,
    patchBytes,
    omittedPatchFiles,
  };
}

export function buildChangelogPrompt({ version, baseRef, changeContext }) {
  const commitLines = changeContext.commits
    .filter((commit) => !isReleaseNoiseCommit(commit.subject))
    .map((commit) => {
      const body = commit.body ? `\n  说明：${indentMultiline(commit.body, '  ')}` : '';
      return `- ${commit.hash} ${commit.subject}${body}`;
    })
    .join('\n');
  const omittedFiles = changeContext.omittedPatchFiles
    .map((file) => `- ${file.path}（${file.reason}）`)
    .join('\n');

  return `请根据以下 Git 提交元数据、文件统计和最终净 Diff，为 motrix-fnos 生成 ${version} 版本的中文 CHANGELOG。

要求：
- 以实际代码和文档变化为依据，不要只改写 commit 标题。
- 合并属于同一功能的多个提交，优先描述用户可感知的最终行为。
- 区分功能、修复、文档、测试与内部重构，不要把测试或工程改动夸大为用户功能。
- 只返回 Markdown 正文，不要返回版本标题。
- 使用这些分组标题中的一种或多种：### 新增、### 改进、### 修复、### 文档。
- 每条使用简洁中文 bullet，不要提及 commit hash。

范围：${baseRef}..HEAD

提交：
${commitLines || '- 无非发版维护提交'}

文件状态：
${changeContext.fileStatus || '（无）'}

增删统计：
${changeContext.numstat || '（无）'}

汇总：
${changeContext.diffStat || '（无）'}

未提供补丁正文的文件：
${omittedFiles || '（无）'}

最终净文本 Diff：
<diff>
${changeContext.patch || '（无文本补丁）'}
</diff>
`;
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

function indentMultiline(value, indent) {
  return value.replace(/\n/g, `\n${indent}`);
}

function isReleaseNoiseCommit(subject) {
  return (
    /^chore:\s*发布\s+\d+\.\d+\.\d+\s+版本/.test(subject)
    || /^发布\s+\d+\.\d+\.\d+\s+版本/.test(subject)
    || /^Update CHANGELOG\.md$/i.test(subject)
  );
}

function git(repoRoot, args, { trim = true } = {}) {
  const output = execFileSync('git', args, {
    cwd: repoRoot,
    encoding: 'utf8',
    maxBuffer: 16 * 1024 * 1024,
  });
  return trim ? output.trim() : output;
}
