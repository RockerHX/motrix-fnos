#!/usr/bin/env node
import { createHash } from 'node:crypto';
import { existsSync, readFileSync, readdirSync, writeFileSync } from 'node:fs';
import path from 'node:path';

const sidecarVersion = '2.8.6';
const sidecarReleaseUrl = `https://github.com/AnInsomniacy/aria2-next/releases/tag/v${sidecarVersion}`;
const sidecarDependencyManifest = `assets/aria2/aria2-next-${sidecarVersion}-dependencies.env`;

export function createSpdxDocument({ version, artifact, sidecar }) {
  const packageId = `SPDXRef-${artifact.name.replace(/[^A-Za-z0-9.-]/g, '-')}`;
  const sidecarId = `SPDXRef-aria2-next-${artifact.architecture}`;
  return {
    spdxVersion: 'SPDX-2.3',
    dataLicense: 'CC0-1.0',
    SPDXID: 'SPDXRef-DOCUMENT',
    name: `${artifact.name}-sbom`,
    documentNamespace: `https://github.com/rockerhx/motrix-fnos/sbom/${version}/${artifact.name}`,
    creationInfo: {
      created: new Date().toISOString(),
      creators: ['Tool: motrix-fnos-sbom/1.0'],
    },
    packages: [
      {
        SPDXID: packageId,
        name: artifact.name,
        versionInfo: version,
        downloadLocation: 'NOASSERTION',
        filesAnalyzed: false,
        supplier: 'NOASSERTION',
        checksums: [{ algorithm: 'SHA256', checksumValue: artifact.sha256 }],
        externalRefs: [
          {
            referenceCategory: 'OTHER',
            referenceType: 'motrix-fnos:architecture',
            referenceLocator: artifact.architecture,
          },
        ],
      },
      {
        SPDXID: sidecarId,
        name: 'aria2-next',
        versionInfo: sidecarVersion,
        downloadLocation: sidecarReleaseUrl,
        filesAnalyzed: false,
        supplier: 'NOASSERTION',
        checksums: [{ algorithm: 'SHA256', checksumValue: sidecar.sha256 }],
        externalRefs: [
          {
            referenceCategory: 'OTHER',
            referenceType: 'motrix-fnos:architecture',
            referenceLocator: artifact.architecture,
          },
          {
            referenceCategory: 'OTHER',
            referenceType: 'motrix-fnos:dependency-manifest',
            referenceLocator: sidecarDependencyManifest,
          },
        ],
      },
    ],
    relationships: [{ spdxElementId: packageId, relationshipType: 'CONTAINS', relatedSpdxElement: sidecarId }],
  };
}

export function sha256File(filePath) {
  return createHash('sha256').update(readFileSync(filePath)).digest('hex');
}

export function writeFpkSboms({ version, outputDir }) {
  const sidecarDir = path.join(process.cwd(), 'assets', 'aria2');
  const dependencyManifestPath = path.join(sidecarDir, `aria2-next-${sidecarVersion}-dependencies.env`);
  if (!existsSync(dependencyManifestPath)) {
    throw new Error(`缺少 Aria2 native 依赖材料：${dependencyManifestPath}`);
  }
  const artifacts = readdirSync(outputDir)
    .filter((file) => file.startsWith(`motrix_${version}_`) && file.endsWith('.fpk'))
    .map((name) => ({
      name,
      architecture: name.endsWith('_arm.fpk') ? 'aarch64' : name.endsWith('_x86.fpk') ? 'x86_64' : null,
      sha256: sha256File(path.join(outputDir, name)),
    }))
    .filter((artifact) => artifact.architecture);

  if (artifacts.length !== 2 || new Set(artifacts.map((artifact) => artifact.architecture)).size !== 2) {
    throw new Error(`SBOM 需要恰好包含 x86_64 和 aarch64 两个 FPK，实际为 ${artifacts.map((artifact) => artifact.name).join(', ')}`);
  }

  for (const artifact of artifacts) {
    const sidecarName = `aria2-next-${artifact.architecture}-unknown-linux-gnu`;
    const sidecarPath = path.join(sidecarDir, sidecarName);
    if (!existsSync(sidecarPath)) {
      throw new Error(`缺少 ${artifact.architecture} Aria2 sidecar：${sidecarPath}`);
    }
    const sbomPath = path.join(outputDir, `${artifact.name}.spdx.json`);
    writeFileSync(
      sbomPath,
      `${JSON.stringify(createSpdxDocument({ version, artifact, sidecar: { sha256: sha256File(sidecarPath) } }), null, 2)}\n`,
    );
  }
  return artifacts.map((artifact) => `${artifact.name}.spdx.json`);
}

function readOption(name) {
  const index = process.argv.indexOf(name);
  return index === -1 ? undefined : process.argv[index + 1];
}

if (process.argv[1]?.endsWith('generate-fpk-sbom.mjs')) {
  const version = readOption('--version');
  const outputDir = readOption('--output-dir') ?? path.join(process.cwd(), 'packaging', 'fnos', 'dist');
  if (!version || !existsSync(outputDir)) {
    console.error('用法：node scripts/release/generate-fpk-sbom.mjs --version <x.y.z> [--output-dir <dir>]');
    process.exit(1);
  }
  try {
    for (const file of writeFpkSboms({ version, outputDir })) {
      console.log(`已生成 SBOM：${path.join(outputDir, file)}`);
    }
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exit(1);
  }
}
