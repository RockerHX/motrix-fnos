import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';

test('日志运行维护文档记录轮转、脱敏和请求关联规则', () => {
  const packagingDoc = readFileSync('docs/fpk-packaging.md', 'utf8');
  const apiDoc = readFileSync('docs/api-contract.md', 'utf8');

  assert.match(packagingDoc, /logs\/server\.log.*10 MiB/);
  assert.match(packagingDoc, /logs\/lifecycle\.log.*1 MiB/);
  assert.match(apiDoc, /URL 的 query\/fragment、Token、密码、Session、CSRF、Cookie、Authorization/);
  assert.match(apiDoc, /X-Request-ID/);
});
