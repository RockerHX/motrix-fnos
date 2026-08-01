import { createHash } from 'node:crypto';

export function compareReleaseVersions(left, right) {
  const leftVersion = parseComparableVersion(left);
  const rightVersion = parseComparableVersion(right);
  for (let index = 0; index < 3; index += 1) {
    if (leftVersion.core[index] !== rightVersion.core[index]) {
      return leftVersion.core[index] - rightVersion.core[index];
    }
  }

  if (leftVersion.isPrerelease === rightVersion.isPrerelease) return 0;
  return leftVersion.isPrerelease ? -1 : 1;
}

function parseComparableVersion(version) {
  const match = version.match(/^(\d+)\.(\d+)\.(\d+)(?:-(beta))?$/);
  if (!match) {
    throw new Error(`无法比较版本号：${version}`);
  }
  return {
    core: match.slice(1, 4).map(Number),
    isPrerelease: match[4] !== undefined,
  };
}

export function normalizeGeneratedChangelog(content) {
  let body = content
    .trim()
    .replace(/^```(?:markdown|md)?\s*/i, '')
    .replace(/```$/i, '')
    .trim();

  body = body.replace(/^##(?!#)\s+[^\n]*\n+/, '').trim();
  if (!body) {
    throw new Error('模型返回了空 CHANGELOG');
  }
  validateChangelogBody(body, '模型生成的 CHANGELOG');
  return body;
}

export function validateChangelogBody(body, source = 'CHANGELOG') {
  const allowedSections = new Set(['新增', '改进', '修复', '文档']);
  let currentSection = null;
  let currentItemCount = 0;
  let totalItemCount = 0;

  for (const rawLine of body.split(/\r?\n/)) {
    const line = rawLine.trim();
    if (!line) continue;

    const sectionMatch = line.match(/^###\s+(.+)$/);
    if (sectionMatch) {
      if (currentSection && currentItemCount === 0) {
        throw new Error(`${source} 的“${currentSection}”分类没有日志条目`);
      }
      const section = sectionMatch[1].trim();
      if (!allowedSections.has(section)) {
        throw new Error(`${source} 包含不允许的分类“${section}”`);
      }
      currentSection = section;
      currentItemCount = 0;
      continue;
    }

    if (line.startsWith('- ')) {
      if (!currentSection) {
        throw new Error(`${source} 包含未归入分类的日志条目：${line}`);
      }
      if (!line.slice(2).trim()) {
        throw new Error(`${source} 的“${currentSection}”分类包含空日志条目`);
      }
      currentItemCount += 1;
      totalItemCount += 1;
      continue;
    }

    throw new Error(`${source} 包含不支持的内容：${line}`);
  }

  if (currentSection && currentItemCount === 0) {
    throw new Error(`${source} 的“${currentSection}”分类没有日志条目`);
  }
  if (totalItemCount === 0) {
    throw new Error(`${source} 没有有效日志条目`);
  }
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

export function validateFpkArtifactName(fileName, version, platform) {
  const expected = `motrix_${version}_${platform}.fpk`;
  if (fileName !== expected) {
    throw new Error(`FPK 产物名必须为 ${expected}，实际为 ${fileName}`);
  }
  return true;
}

export function validateFpkRuntimeDataEntries(entries) {
  const leftovers = entries.filter((entry) => entry !== '.gitkeep');
  if (leftovers.length > 0) {
    throw new Error(`FPK app/data 不得包含运行时残留：${leftovers.join(', ')}`);
  }
  return true;
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

export function resolveFpkEntryId(config, preferredEntryId) {
  if (preferredEntryId) {
    return preferredEntryId;
  }

  const entryIds = Object.keys(config?.['.url'] ?? {});
  if (entryIds.length !== 1) {
    throw new Error(`manifest 未指定 desktop_applaunchname 时必须且只能配置一个应用入口，实际为 ${entryIds.length} 个`);
  }
  return entryIds[0];
}

export function validateFpkAppIdentity({
  manifestContent,
  uiConfig,
  expectedAppName,
  expectedEntryId,
}) {
  const manifest = parseManifest(manifestContent);
  if (manifest.appname !== expectedAppName) {
    throw new Error(`manifest.appname 必须为 ${expectedAppName}，实际为 ${manifest.appname ?? '(missing)'}`);
  }
  if (manifest.desktop_appname !== expectedEntryId) {
    throw new Error(`manifest.desktop_appname 必须为 ${expectedEntryId}，实际为 ${manifest.desktop_appname ?? '(missing)'}`);
  }

  const launchName = manifestContent.match(/^desktop_applaunchname\s*=([^\r\n]*)$/m);
  if (!launchName || launchName[1].trim()) {
    throw new Error('manifest.desktop_applaunchname 必须保留为空');
  }

  const entryIds = Object.keys(uiConfig?.['.url'] ?? {});
  if (entryIds.length !== 1 || entryIds[0] !== expectedEntryId) {
    throw new Error(`应用入口必须且只能为 ${expectedEntryId}，实际为 ${entryIds.join(', ') || '(missing)'}`);
  }
}

export function validateFpkPortIsolation({
  manifestContent,
  uiConfig,
  portConfigContent,
  resourceContent,
  managementPort,
  jsonRpcPort,
  lanJsonRpcPort,
}) {
  const manifest = parseManifest(manifestContent);
  if (manifest.service_port !== managementPort) {
    throw new Error(`manifest.service_port 必须为管理端口 ${managementPort}，实际为 ${manifest.service_port ?? '(missing)'}`);
  }
  const entryId = resolveFpkEntryId(uiConfig, manifest.desktop_applaunchname);

  validatePortEntry(uiConfig, {
    entryId,
    port: managementPort,
    url: `/?v=${manifest.version}`,
    accessPerm: 'editable',
  });

  assertNoPort(manifestContent, jsonRpcPort, 'manifest');
  assertNoPort(JSON.stringify(uiConfig), jsonRpcPort, '应用入口配置');
  assertNoPort(resourceContent, jsonRpcPort, 'config/resource');
  assertNoPort(portConfigContent, jsonRpcPort, 'MotrixFNOS.sc');
  assertNoPort(manifestContent, lanJsonRpcPort, 'manifest');
  assertNoPort(JSON.stringify(uiConfig), lanJsonRpcPort, '应用入口配置');
  assertNoPort(resourceContent, lanJsonRpcPort, 'config/resource');

  const expectedPort = `${managementPort}/tcp,${lanJsonRpcPort}/tcp`;
  for (const key of ['src.ports', 'dst.ports']) {
    const value = readShellAssignment(portConfigContent, key);
    if (value !== expectedPort) {
      throw new Error(`MotrixFNOS.sc ${key} 必须为 ${expectedPort}，实际为 ${value ?? '(missing)'}`);
    }
  }
}

export function validateFpkRuntimeEnvScript(content, expectedJsonRpcAddr, expectedLanJsonRpcAddr) {
  const defaultLine = `JSONRPC_ADDR=\${MOTRIX_FNOS_JSONRPC_ADDR:-"${expectedJsonRpcAddr}"}`;
  if (!content.includes(defaultLine)) {
    throw new Error(`cmd/common.sh 缺少 JSON-RPC 回环默认值 ${expectedJsonRpcAddr}`);
  }
  if (!content.includes('export MOTRIX_FNOS_JSONRPC_ADDR="${JSONRPC_ADDR}"')) {
    throw new Error('cmd/common.sh 未导出 MOTRIX_FNOS_JSONRPC_ADDR');
  }
  const lanDefaultLine = `LAN_JSONRPC_ADDR=\${MOTRIX_FNOS_LAN_JSONRPC_ADDR:-"${expectedLanJsonRpcAddr}"}`;
  if (!content.includes(lanDefaultLine)) {
    throw new Error(`cmd/common.sh 缺少局域网 JSON-RPC 默认值 ${expectedLanJsonRpcAddr}`);
  }
  if (!content.includes('export MOTRIX_FNOS_LAN_JSONRPC_ADDR="${LAN_JSONRPC_ADDR}"')) {
    throw new Error('cmd/common.sh 未导出 MOTRIX_FNOS_LAN_JSONRPC_ADDR');
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

function assertNoPort(content, port, label) {
  const pattern = new RegExp(`(^|[^0-9])${escapeRegExp(port)}([^0-9]|$)`);
  if (pattern.test(content)) {
    throw new Error(`${label} 不得声明 JSON-RPC 专用端口 ${port}`);
  }
}

function readShellAssignment(content, key) {
  const pattern = new RegExp(`^${escapeRegExp(key)}="?([^"\\r\\n]+)"?$`, 'm');
  return content.match(pattern)?.[1];
}
