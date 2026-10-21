import assert from 'node:assert/strict';
import { execFileSync, spawnSync } from 'node:child_process';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

const scriptPath = fileURLToPath(new URL('../release/release-notes.mjs', import.meta.url));

test('GitHub Release 正文只接受分类完整的目标版本 CHANGELOG', () => {
  const repoRoot = mkdtempSync(path.join(os.tmpdir(), 'motrix-release-notes-'));
  try {
    writeFileSync(
      path.join(repoRoot, 'CHANGELOG.md'),
      '# Changelog\n\n## 2.0.0 - 2026-07-15\n\n### 修复\n\n- 修复更新日志。\n',
    );
    const output = execFileSync(process.execPath, [scriptPath, '2.0.0'], { cwd: repoRoot, encoding: 'utf8' });
    assert.match(output, /### 修复\n\n- 修复更新日志。/);

    writeFileSync(
      path.join(repoRoot, 'CHANGELOG.md'),
      '# Changelog\n\n## 2.0.0 - 2026-07-15\n\n- 未分类日志。\n\n### 修复\n\n- 修复更新日志。\n',
    );
    const invalid = spawnSync(process.execPath, [scriptPath, '2.0.0'], { cwd: repoRoot, encoding: 'utf8' });
    assert.notEqual(invalid.status, 0);
    assert.match(invalid.stderr, /未归入分类的日志条目/);
  } finally {
    rmSync(repoRoot, { recursive: true, force: true });
  }
});

test('2.0.0 发布说明固定包含旧任务限制与完整回滚步骤，提交正文保持 CHANGELOG 内容', () => {
  const repoRoot = mkdtempSync(path.join(os.tmpdir(), 'motrix-release-notes-'));
  try {
    const body = '### 新增\n\n- 升级下载引擎。';
    writeFileSync(path.join(repoRoot, 'CHANGELOG.md'), `# Changelog\n\n## 2.0.0 - 2026-10-21\n\n${body}\n`);
    const output = execFileSync(process.execPath, [scriptPath, '2.0.0'], { cwd: repoRoot, encoding: 'utf8' });
    assert.ok(output.includes('Motrix 2.0.0 使用 Aria2 Next 2.8.6'));
    assert.ok(output.includes('2.0.0 以下版本创建的任务'));
    assert.ok(!output.includes('2.9.0'));
    for (const text of ['不支持继续或恢复', '旧任务记录仅供查看', '整个应用私有数据目录', '不要依赖安装器自动回退', 'SHA256SUMS.txt', '不能只换引擎', '新版新增任务不能由旧快照继续', 'Content-Range', '拒绝删除下载内容', '最近未提交的进度可能回退', 'SOCKS5', 'Header 不会传递']) {
      assert.ok(output.includes(text), text);
    }
    assert.match(output, /https:\/\/github.com\/RockerHX\/motrix-fnos\/releases\/tag\/v1\.9\.8/);
    const commitBody = execFileSync(process.execPath, [scriptPath, '2.0.0', '--body'], { cwd: repoRoot, encoding: 'utf8' });
    assert.equal(commitBody.trim(), body);
    writeFileSync(path.join(repoRoot, 'CHANGELOG.md'), `# Changelog\n\n## 2.0.1 - 2026-10-22\n\n${body}\n`);
    const next = execFileSync(process.execPath, [scriptPath, '2.0.1'], { cwd: repoRoot, encoding: 'utf8' });
    assert.doesNotMatch(next, /升级前必读|v1\.9\.8/);
  } finally {
    rmSync(repoRoot, { recursive: true, force: true });
  }
});
