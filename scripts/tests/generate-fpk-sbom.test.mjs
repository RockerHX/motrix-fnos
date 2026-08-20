import assert from 'node:assert/strict';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { createSpdxDocument, writeFpkSboms } from '../release/generate-fpk-sbom.mjs';

test('双架构 FPK 生成带 SHA-256 和架构信息的 SPDX SBOM', () => {
  const outputDir = mkdtempSync(path.join(tmpdir(), 'motrix-fpk-sbom-'));
  try {
    writeFileSync(path.join(outputDir, 'motrix_1.8.1_x86.fpk'), 'x86 package');
    writeFileSync(path.join(outputDir, 'motrix_1.8.1_arm.fpk'), 'arm package');

    const files = writeFpkSboms({ version: '1.8.1', outputDir });

    assert.deepEqual(files.sort(), ['motrix_1.8.1_arm.fpk.spdx.json', 'motrix_1.8.1_x86.fpk.spdx.json']);
    const sbom = JSON.parse(readFileSync(path.join(outputDir, files[0]), 'utf8'));
    assert.equal(sbom.spdxVersion, 'SPDX-2.3');
    assert.equal(sbom.packages.length, 1);
    assert.match(sbom.packages[0].checksums[0].checksumValue, /^[a-f0-9]{64}$/);
    assert.match(sbom.packages[0].externalRefs[0].referenceLocator, /^(x86_64|aarch64)$/);
  } finally {
    rmSync(outputDir, { recursive: true, force: true });
  }
});

test('SBOM 文档保留固定 SPDX 基本字段', () => {
  const document = createSpdxDocument({
    version: '1.8.1',
    artifact: { name: 'motrix_1.8.1_x86.fpk', architecture: 'x86_64', sha256: 'a'.repeat(64) },
  });

  assert.equal(document.dataLicense, 'CC0-1.0');
  assert.equal(document.packages[0].filesAnalyzed, false);
  assert.equal(document.packages[0].checksums[0].algorithm, 'SHA256');
});
