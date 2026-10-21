#!/usr/bin/env node
import { readFileSync } from 'node:fs';
import path from 'node:path';
import process from 'node:process';
import { validateChangelogBody } from '../lib/script-utils.mjs';

const version = process.argv[2];
const bodyOnly = process.argv.includes('--body');
const repoRoot = process.cwd();

if (!version) {
  console.error('用法：node scripts/release/release-notes.mjs <x.y.z> [--body]');
  process.exit(1);
}

try {
  const section = readChangelogSection(version);
  if (bodyOnly) {
    console.log(section.body);
  } else {
    console.log(section.section);
    if (version === '2.9.0') {
      console.log(`
## 升级前必读

Motrix 2.9.0 使用 Aria2 Next 2.8.6，不支持继续或恢复 2.9.0 以下版本创建的任务。请先在旧版完成所有下载，包括暂停、失败、等待选择文件以及回收站中需要继续的任务。升级后旧任务记录仅供查看；下载文件、管理密码、授权、设置和 RPC Token 保留。

升级前请停止应用并确认 server 与引擎已退出，备份整个应用私有数据目录（含 SQLite、session、原生状态和种子备份），记录旧包版本、架构及备份校验和；下载内容保持原位。升级失败时请保留原数据，并查看安装提示；不要依赖安装器自动回退。

## 已知限制

- 固定的 Aria2 Next 2.8.6 遇到某些错误的 Content-Range 响应时，可能一直显示下载中且无法暂停。应用在无法确认暂停时返回明确错误并保留原任务和文件，拒绝删除下载内容；请通过飞牛停止并重新启动应用，再核对任务和文件。停服可能需要强制终止引擎，最近未提交的进度可能回退，不代表完整下载成功；不要手动删除 session 或原生恢复状态。
- BT tracker/web-seed 使用 HTTP 或 SOCKS4 代理时，部分主机名解析可能依赖本机 DNS；需要由代理解析目标主机名时建议使用 SOCKS5。BT 的自定义 HTTP Header 不会传递给 tracker/web-seed，依赖这些 Header 认证的来源暂不支持。

## 需要回滚时

1. 停止新版并确认 server 与引擎已退出，另存新版新增任务状态及下载文件归属。
2. 下载与设备架构匹配的 [Motrix v1.9.8 稳定恢复包](https://github.com/RockerHX/motrix-fnos/releases/tag/v1.9.8)，按该 Release 的 SHA256SUMS.txt 核对 FPK。
3. 配套恢复升级前的整个应用私有数据目录并安装原版本；原实例不是 v1.9.8 时须使用与备份匹配的原版本包，不能只换引擎或让旧引擎加载新 session。
4. 启动旧版后核对任务、密码、授权、设置、RPC Token 和下载文件。新版新增任务不能由旧快照继续；没有升级前完整备份时，先保留现有数据，不要反复切换版本。
`);
    }
  }
} catch (error) {
  console.error(error instanceof Error ? error.message : String(error));
  process.exit(1);
}

function readChangelogSection(targetVersion) {
  const changelogPath = path.join(repoRoot, 'CHANGELOG.md');
  const content = readFileSync(changelogPath, 'utf8');
  const lines = content.split(/\r?\n/);
  const start = lines.findIndex((line) => new RegExp(`^##\\s+${escapeRegExp(targetVersion)}\\b`).test(line));
  if (start === -1) {
    throw new Error(`CHANGELOG.md 缺少 ${targetVersion} 条目`);
  }

  const next = lines.findIndex((line, index) => index > start && /^##\s+/.test(line));
  const sectionLines = lines.slice(start, next === -1 ? undefined : next);
  const section = sectionLines.join('\n').trimEnd();
  const body = section.replace(/^##[^\n]*\n*/, '').trim();
  if (!body) {
    throw new Error(`CHANGELOG.md 中 ${targetVersion} 条目为空`);
  }
  validateChangelogBody(body, `CHANGELOG.md 中 ${targetVersion} 条目`);

  return { section, body };
}

function escapeRegExp(value) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}
