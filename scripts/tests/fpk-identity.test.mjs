import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';
import { validateFpkAppIdentity } from '../script-utils.mjs';

test('仓库 FPK 身份与 Release 产物名保持一致', () => {
  const manifestContent = readFileSync('packaging/fnos/manifest.template', 'utf8');
  const uiConfig = JSON.parse(readFileSync('packaging/fnos/app/ui/config', 'utf8'));
  const releaseWorkflow = readFileSync('.github/workflows/release.yml', 'utf8');

  assert.doesNotThrow(() =>
    validateFpkAppIdentity({
      manifestContent,
      uiConfig,
      expectedAppName: 'motrix',
      expectedEntryId: 'motrix.Application',
    }),
  );
  assert.match(releaseWorkflow, /motrix_\$\{VERSION\}_x86\.fpk/);
  assert.match(releaseWorkflow, /motrix_\$\{VERSION\}_arm\.fpk/);
  assert.doesNotMatch(releaseWorkflow, /motrix\.fnos_\$\{VERSION\}/);
});
