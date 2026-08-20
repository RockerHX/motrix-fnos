import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import test from 'node:test';

const repoRoot = process.cwd();

test('首屏在 Vue 和鉴权代码加载前显示无依赖反馈', () => {
  const html = readFileSync(path.join(repoRoot, 'index.html'), 'utf8');

  assert.match(html, /<main id="app-bootstrap" class="app-bootstrap"[^>]*role="status"/);
  assert.match(html, /id="app-bootstrap-status">正在建立安全连接/);
  assert.match(html, /@keyframes app-bootstrap-spin/);
  assert.match(html, /prefers-reduced-motion: reduce/);
  assert.ok(html.indexOf('class="app-bootstrap"') < html.indexOf('src="/src/main.ts"'));
  assert.doesNotMatch(html, /app-bootstrap[^>]+src=/);
});
