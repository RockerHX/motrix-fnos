import { readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';

export const repoRoot = process.cwd();

export const versionFiles = {
  packageJson: path.join(repoRoot, 'package.json'),
  cargoToml: path.join(repoRoot, 'server', 'Cargo.toml'),
  manifestTemplate: path.join(repoRoot, 'packaging', 'fnos', 'manifest.template'),
};

export function assertReleaseVersion(version) {
  if (!/^\d+\.\d+\.\d+$/.test(version)) {
    throw new Error(`版本号必须使用 x.y.z 格式，实际为：${version}`);
  }
}

export function readProjectVersions() {
  const packageJson = readJson(versionFiles.packageJson);
  const cargoToml = readText(versionFiles.cargoToml);
  const manifestTemplate = readText(versionFiles.manifestTemplate);

  return {
    packageJson: packageJson.version,
    cargoToml: matchRequired(cargoToml, /^version\s*=\s*"([^"]+)"/m, versionFiles.cargoToml),
    manifestTemplate: matchRequired(manifestTemplate, /^version\s*=\s*(\S+)/m, versionFiles.manifestTemplate),
  };
}

export function setProjectVersion(version) {
  assertReleaseVersion(version);

  const packageJson = readJson(versionFiles.packageJson);
  packageJson.version = version;
  writeJson(versionFiles.packageJson, packageJson);

  writeText(
    versionFiles.cargoToml,
    replaceRequired(
      readText(versionFiles.cargoToml),
      /^version\s*=\s*"[^"]+"/m,
      `version = "${version}"`,
      versionFiles.cargoToml,
    ),
  );

  writeText(
    versionFiles.manifestTemplate,
    replaceRequired(
      readText(versionFiles.manifestTemplate),
      /^version\s*=.*$/m,
      `version               = ${version}`,
      versionFiles.manifestTemplate,
    ),
  );

}

export function findVersionMismatches(versions) {
  const expected = versions.packageJson;
  return Object.entries(versions)
    .filter(([, version]) => version !== expected)
    .map(([source, version]) => ({ source, version, expected }));
}

function readText(filePath) {
  return readFileSync(filePath, 'utf8');
}

function writeText(filePath, content) {
  writeFileSync(filePath, content);
}

function readJson(filePath) {
  return JSON.parse(readText(filePath));
}

function writeJson(filePath, value) {
  writeText(filePath, `${JSON.stringify(value, null, 2)}\n`);
}

function matchRequired(content, pattern, filePath) {
  const match = content.match(pattern);
  if (!match) {
    throw new Error(`无法读取版本号：${filePath}`);
  }
  return match[1];
}

function replaceRequired(content, pattern, replacement, filePath) {
  if (!pattern.test(content)) {
    throw new Error(`无法更新版本号：${filePath}`);
  }
  return content.replace(pattern, replacement);
}
