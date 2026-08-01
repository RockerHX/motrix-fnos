import { readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';

export const repoRoot = process.cwd();

export const versionFiles = {
  packageJson: path.join(repoRoot, 'package.json'),
  cargoToml: path.join(repoRoot, 'server', 'Cargo.toml'),
  manifestTemplate: path.join(repoRoot, 'packaging', 'fnos', 'manifest.template'),
  uiConfig: path.join(repoRoot, 'packaging', 'fnos', 'app', 'ui', 'config'),
};

const releaseVersionPattern = /^\d+\.\d+\.\d+$/;
const projectVersionPattern = /^\d+\.\d+\.\d+(?:-beta)?$/;

export function assertReleaseVersion(version) {
  if (!releaseVersionPattern.test(version)) {
    throw new Error(`版本号必须使用 x.y.z 格式，实际为：${version}`);
  }
}

export function assertProjectVersion(version) {
  if (!projectVersionPattern.test(version)) {
    throw new Error(`项目版本号必须使用 x.y.z 或 x.y.z-beta 格式，实际为：${version}`);
  }
}

export function readProjectVersions(files = versionFiles) {
  const packageJson = readJson(files.packageJson);
  const cargoToml = readText(files.cargoToml);
  const manifestTemplate = readText(files.manifestTemplate);
  const uiConfig = readJson(files.uiConfig);

  return {
    packageJson: packageJson.version,
    cargoToml: matchRequired(cargoToml, /^version\s*=\s*"([^"]+)"/m, files.cargoToml),
    manifestTemplate: matchRequired(manifestTemplate, /^version\s*=\s*(\S+)/m, files.manifestTemplate),
    uiConfig: readUiCacheVersion(uiConfig, files.uiConfig),
  };
}

export function setProjectVersion(version, files = versionFiles) {
  assertProjectVersion(version);

  const packageJson = readJson(files.packageJson);
  packageJson.version = version;
  writeJson(files.packageJson, packageJson);

  writeText(
    files.cargoToml,
    replaceRequired(
      readText(files.cargoToml),
      /^version\s*=\s*"[^"]+"/m,
      `version = "${version}"`,
      files.cargoToml,
    ),
  );

  writeText(
    files.manifestTemplate,
    replaceRequired(
      readText(files.manifestTemplate),
      /^version\s*=.*$/m,
      `version               = ${version}`,
      files.manifestTemplate,
    ),
  );

  const uiConfig = readJson(files.uiConfig);
  const uiEntry = readUiEntry(uiConfig, files.uiConfig);
  uiEntry.url = `/?v=${version}`;
  writeJson(files.uiConfig, uiConfig);
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

function readUiCacheVersion(uiConfig, filePath) {
  const url = readUiEntry(uiConfig, filePath).url;
  if (typeof url !== 'string') {
    throw new Error(`无法读取版本号：${filePath}`);
  }
  return matchRequired(url, /^\/\?v=(\d+\.\d+\.\d+(?:-beta)?)$/, filePath);
}

function readUiEntry(uiConfig, filePath) {
  const entry = uiConfig?.['.url']?.['motrix.Application'];
  if (!entry || typeof entry !== 'object') {
    throw new Error(`无法读取版本号：${filePath}`);
  }
  return entry;
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
