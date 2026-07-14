import assert from 'node:assert/strict';
import { execFileSync, spawnSync } from 'node:child_process';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

const scriptPath = fileURLToPath(new URL('../release-notes.mjs', import.meta.url));

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
