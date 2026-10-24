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
    assert.equal(sbom.packages.length, 2);
    assert.match(sbom.packages[0].checksums[0].checksumValue, /^[a-f0-9]{64}$/);
    assert.match(sbom.packages[0].externalRefs[0].referenceLocator, /^(x86_64|aarch64)$/);
    assert.equal(sbom.packages[1].name, 'aria2-next');
    assert.equal(sbom.packages[1].versionInfo, '2.8.6');
    assert.match(sbom.packages[1].checksums[0].checksumValue, /^[a-f0-9]{64}$/);
    assert.equal(sbom.packages[1].externalRefs[1].referenceLocator, 'assets/aria2/aria2-next-2.8.6-dependencies.env');
    assert.equal(sbom.relationships[0].relationshipType, 'CONTAINS');
  } finally {
    rmSync(outputDir, { recursive: true, force: true });
  }
});

test('SBOM 文档保留固定 SPDX 基本字段', () => {
  const document = createSpdxDocument({
    version: '1.8.1',
    artifact: { name: 'motrix_1.8.1_x86.fpk', architecture: 'x86_64', sha256: 'a'.repeat(64) },
    sidecar: { sha256: 'b'.repeat(64) },
  });

  assert.equal(document.dataLicense, 'CC0-1.0');
  assert.equal(document.packages[0].filesAnalyzed, false);
  assert.equal(document.packages[1].filesAnalyzed, false);
  assert.equal(document.packages[0].checksums[0].algorithm, 'SHA256');
});
