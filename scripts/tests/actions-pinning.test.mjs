import assert from 'node:assert/strict';
import { readdirSync, readFileSync } from 'node:fs';
import path from 'node:path';
import test from 'node:test';

test('所有 GitHub Actions 引用均固定为带版本注释的 commit SHA', () => {
  const workflowDir = '.github/workflows';
  const workflowFiles = readdirSync(workflowDir).filter((file) => file.endsWith('.yml') || file.endsWith('.yaml'));
  const actionReferences = [];

  for (const file of workflowFiles) {
    const source = readFileSync(path.join(workflowDir, file), 'utf8');
    for (const line of source.split(/\r?\n/)) {
      const match = line.match(/^\s*uses:\s*([^@\s]+)@([^\s#]+)(?:\s+#\s*(.+))?\s*$/);
      if (match) actionReferences.push({ file, line, reference: match });
    }
  }

  assert.ok(actionReferences.length > 0, '至少应检查一个 GitHub Actions 引用');
  for (const { file, line, reference } of actionReferences) {
    assert.match(reference[2], /^[0-9a-f]{40}$/i, `${file} 的 action 未固定 commit SHA：${line}`);
    assert.ok(reference[3]?.trim(), `${file} 的 action 缺少版本注释：${line}`);
  }
});
