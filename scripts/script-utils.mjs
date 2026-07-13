import { createHash } from 'node:crypto';

export function compareReleaseVersions(left, right) {
  const leftParts = left.split('.').map(Number);
  const rightParts = right.split('.').map(Number);
  for (let index = 0; index < 3; index += 1) {
    if (leftParts[index] !== rightParts[index]) {
      return leftParts[index] - rightParts[index];
    }
  }
  return 0;
}

export function normalizeGeneratedChangelog(content) {
  let body = content
    .trim()
    .replace(/^```(?:markdown|md)?\s*/i, '')
    .replace(/```$/i, '')
    .trim();

  body = body.replace(/^##[^\n]*\n+/, '').trim();
  if (!body) {
    throw new Error('模型返回了空 CHANGELOG');
  }
  if (!/^###\s+/m.test(body)) {
    body = `### 改进\n\n${body}`;
  }
  return body;
}

export function classifyCommit(subject) {
  if (/^(feat|新增)(\(.+\))?:/i.test(subject)) return '新增';
  if (/^(fix|修复)(\(.+\))?:/i.test(subject)) return '修复';
  if (/^(docs|文档)(\(.+\))?:/i.test(subject)) return '文档';
  return '改进';
}

export function cleanupCommitSubject(subject) {
  return subject
    .replace(/^(feat|fix|docs|chore|ci|build|refactor|perf|test)(\(.+\))?:\s*/i, '')
    .trim();
}

export function platformForTarget(target) {
  return target === 'aarch64-unknown-linux-gnu' ? 'arm' : 'x86';
}

export function upsertManifestField(content, key, value) {
  const line = `${key.padEnd(22, ' ')}= ${value}`;
  const pattern = new RegExp(`^${escapeRegExp(key)}\\s*=.*$`, 'm');
  if (pattern.test(content)) {
    return content.replace(pattern, line);
  }

  const sourcePattern = /^source\s*=.*$/m;
  if (sourcePattern.test(content)) {
    return content.replace(sourcePattern, (sourceLine) => `${sourceLine}\n${line}`);
  }

  return `${content.trimEnd()}\n${line}\n`;
}

export function removeManifestField(content, key) {
  const pattern = new RegExp(`^${escapeRegExp(key)}\\s*=.*\\r?\\n?`, 'm');
  return content.replace(pattern, '');
}

export function parseManifest(content) {
  return Object.fromEntries(
    content
      .split(/\r?\n/)
      .map((line) => line.match(/^([^#=]+?)\s*=\s*(.+)$/))
      .filter(Boolean)
      .map(([, key, value]) => [key.trim(), value.trim()]),
  );
}

export function validatePortEntry(config, expected) {
  const entry = config?.['.url']?.[expected.entryId];
  if (!entry || typeof entry !== 'object') {
    throw new Error(`缺少端口入口 ${expected.entryId}`);
  }
  if (entry.type !== 'iframe') {
    throw new Error(`端口入口 type 必须为 iframe，实际为 ${entry.type ?? '(missing)'}`);
  }
  if (entry.protocol !== 'http') {
    throw new Error(`端口入口 protocol 必须为 http，实际为 ${entry.protocol ?? '(missing)'}`);
  }
  if (entry.port !== expected.port) {
    throw new Error(`端口入口 port 必须为 ${expected.port}，实际为 ${entry.port ?? '(missing)'}`);
  }
  if (Object.prototype.hasOwnProperty.call(entry, 'gatewayPrefix') || Object.prototype.hasOwnProperty.call(entry, 'gatewaySocket')) {
    throw new Error('端口入口不得声明 gatewayPrefix 或 gatewaySocket');
  }
  if (entry.url !== expected.url) {
    throw new Error(`端口入口 url 必须为 ${expected.url}，实际为 ${entry.url ?? '(missing)'}`);
  }
  if (entry.control?.accessPerm !== expected.accessPerm) {
    throw new Error(`端口入口 control.accessPerm 必须为 ${expected.accessPerm}，实际为 ${entry.control?.accessPerm ?? '(missing)'}`);
  }
  if (entry.control?.portPerm !== 'readonly') {
    throw new Error(`端口入口 control.portPerm 必须为 readonly，实际为 ${entry.control?.portPerm ?? '(missing)'}`);
  }
}

export function parseChecksums(text) {
  const result = new Map();
  for (const line of text.split(/\r?\n/)) {
    const match = line.trim().match(/^([a-f0-9]{64})\s+(.+)$/i);
    if (match) {
      result.set(match[2].trim(), match[1].toLowerCase());
    }
  }
  return result;
}

export function sha256(buffer) {
  return createHash('sha256').update(buffer).digest('hex');
}

function escapeRegExp(value) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}
