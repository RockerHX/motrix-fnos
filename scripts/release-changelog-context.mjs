import { execFileSync } from 'node:child_process';

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

/**
 * 收集发布日志所需的 commit 上下文和本地文件统计。
 * 不读取或返回源码 Diff，避免把同一批变更重复发送给模型。
 */
export function collectReleaseCommitContext({
  repoRoot,
  baseRef,
  headRef = 'HEAD',
  commits = readReleaseCommits(repoRoot, baseRef, headRef),
}) {
  const range = `${baseRef}..${headRef}`;
  return {
    commits,
    fileStatus: git(repoRoot, ['diff', '--no-renames', '--name-status', range]),
    numstat: git(repoRoot, ['diff', '--no-renames', '--numstat', range]),
    diffStat: git(repoRoot, ['diff', '--no-renames', '--stat', '--summary', range]),
  };
}

function git(repoRoot, args) {
  return execFileSync('git', args, {
    cwd: repoRoot,
    encoding: 'utf8',
    maxBuffer: 16 * 1024 * 1024,
  }).trim();
}
