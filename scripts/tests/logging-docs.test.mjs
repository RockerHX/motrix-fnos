import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';

test('日志运行维护文档记录轮转、脱敏和请求关联规则', () => {
  const maintenanceDoc = readFileSync('docs/maintenance.md', 'utf8');
  const apiDoc = readFileSync('docs/api-contract.md', 'utf8');

  assert.match(maintenanceDoc, /logs\/server\.log.*10 MiB/);
  assert.match(maintenanceDoc, /logs\/lifecycle\.log.*1 MiB/);
  assert.match(apiDoc, /URL 的 query\/fragment、JWT、各类 Token、密码、Authorization/);
  assert.match(apiDoc, /X-Request-ID/);
});
