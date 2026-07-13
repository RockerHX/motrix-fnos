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

export function validateGatewayEntry(config, expected) {
  const entry = config?.['.url']?.[expected.entryId];
  if (!entry || typeof entry !== 'object') {
    throw new Error(`缺少统一网关入口 ${expected.entryId}`);
  }
  if (entry.type !== 'iframe') {
    throw new Error(`统一网关入口 type 必须为 iframe，实际为 ${entry.type ?? '(missing)'}`);
  }
  if (entry.protocol !== '') {
    throw new Error(`统一网关入口 protocol 必须为空字符串，实际为 ${entry.protocol ?? '(missing)'}`);
  }
  if (Object.prototype.hasOwnProperty.call(entry, 'port')) {
    throw new Error('统一网关入口不得声明 port，否则 fnOS 可能退回直连服务端口');
  }
  if (entry.gatewayPrefix !== expected.gatewayPrefix) {
    throw new Error(`统一网关入口 gatewayPrefix 必须为 ${expected.gatewayPrefix}，实际为 ${entry.gatewayPrefix ?? '(missing)'}`);
  }
  if (entry.gatewaySocket !== expected.gatewaySocket) {
    throw new Error(`统一网关入口 gatewaySocket 必须为 ${expected.gatewaySocket}，实际为 ${entry.gatewaySocket ?? '(missing)'}`);
  }
  if (entry.url !== expected.url) {
    throw new Error(`统一网关入口 url 必须为稳定路径 ${expected.url}，实际为 ${entry.url ?? '(missing)'}`);
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
