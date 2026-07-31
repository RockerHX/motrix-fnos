import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

const cleanRustTargetScript = fileURLToPath(new URL('../clean-rust-target.mjs', import.meta.url));

test('验证保留 Rust 缓存，完整清理由显式 clean:rust 执行', () => {
  const packageJson = JSON.parse(readFileSync('package.json', 'utf8'));
  const verifyScript = readFileSync('scripts/verify.mjs', 'utf8');

  assert.equal(packageJson.scripts['clean:rust:incremental'], undefined);
  assert.doesNotMatch(verifyScript, /clean-rust-target\.mjs|keep-rust-incremental|incremental/);

  const repoRoot = mkdtempSync(path.join(os.tmpdir(), 'motrix-rust-cache-policy-'));
  try {
    const targetRoot = path.join(repoRoot, 'server', 'target');
    mkdirSync(path.join(targetRoot, 'debug', 'deps'), { recursive: true });
    mkdirSync(path.join(targetRoot, 'debug', 'incremental'), { recursive: true });
    writeFileSync(path.join(targetRoot, 'debug', 'deps', 'artifact'), 'compiled');
    writeFileSync(path.join(targetRoot, 'debug', 'incremental', 'state'), 'incremental');

    execFileSync(process.execPath, [cleanRustTargetScript], { cwd: repoRoot, stdio: 'pipe' });

    assert.equal(existsSync(targetRoot), false);
  } finally {
    rmSync(repoRoot, { recursive: true, force: true });
  }
});
