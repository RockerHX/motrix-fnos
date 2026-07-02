#!/usr/bin/env node
import { chmodSync, copyFileSync, cpSync, existsSync, mkdirSync, readFileSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import os from 'node:os';
import path from 'node:path';
import process from 'node:process';

const repoRoot = process.cwd();
const packagingRoot = path.join(repoRoot, 'packaging', 'fnos');
const manifestPath = path.join(packagingRoot, 'manifest');
const uiConfigPath = path.join(packagingRoot, 'app', 'ui', 'config');
const outputDir = path.join(packagingRoot, 'dist');
const buildTarget = readOption('--target') ?? 'x86_64-unknown-linux-gnu';
const platform = buildTarget === 'aarch64-unknown-linux-gnu' ? 'arm' : 'x86';
const sidecarTarget = buildTarget;
const prepareOnly = process.argv.includes('--prepare-only');
const servicePort = readOption('--service-port') ?? '17080';
const env = {
  ...process.env,
  PATH: [path.join(os.homedir(), '.cargo', 'bin'), path.join(os.homedir(), '.local', 'bin'), process.env.PATH ?? ''].filter(Boolean).join(path.delimiter),
};

const manifestOriginal = readFileSync(manifestPath, 'utf8');
const uiConfigOriginal = readFileSync(uiConfigPath, 'utf8');

try {
  resetAppDataDir();
  run('node', ['scripts/build-server-linux.mjs', '--target', buildTarget], env);
  run('node', ['scripts/build-web-ui-fpk.mjs'], env);
  run('node', ['scripts/stage-aria2-sidecar.mjs', '--target', sidecarTarget], env);
  stageServerBinary(buildTarget);
  syncUiIcons();
  patchManifest(platform, servicePort);
  patchUiConfig(servicePort);

  if (prepareOnly) {
    console.log('FPK 预组装完成，已跳过 fnpack build');
    process.exit(0);
  }

  const fnpack = ensureFnpack(env);
  run(fnpack, ['build'], env, packagingRoot);
  moveOutputFile();
} finally {
  writeFileSync(manifestPath, manifestOriginal);
  writeFileSync(uiConfigPath, uiConfigOriginal);
}

function resetAppDataDir() {
  const dataDir = path.join(packagingRoot, 'app', 'data');
  mkdirSync(dataDir, { recursive: true });
  for (const entry of readdirSync(dataDir)) {
    rmSync(path.join(dataDir, entry), { recursive: true, force: true });
  }
  writeFileSync(path.join(dataDir, '.gitkeep'), '# 占位文件，供 Git 跟踪空目录\n');
}

function stageServerBinary(target) {
  const source = path.join(repoRoot, 'server', 'target', target, 'release', 'motrix-fnos-server');
  const destinationDir = path.join(packagingRoot, 'app', 'bin');
  const destination = path.join(destinationDir, 'motrix-fnos-server');
  if (!existsSync(source)) {
    fail(`缺少 server 构建产物：${source}`);
  }
  mkdirSync(destinationDir, { recursive: true });
  copyFileSync(source, destination);
  chmodSync(destination, 0o755);
}

function syncUiIcons() {
  const imagesDir = path.join(packagingRoot, 'app', 'ui', 'images');
  mkdirSync(imagesDir, { recursive: true });
  copyFileSync(path.join(packagingRoot, 'ICON.PNG'), path.join(imagesDir, 'icon-128.png'));
  copyFileSync(path.join(packagingRoot, 'ICON_256.PNG'), path.join(imagesDir, 'icon-256.png'));
}

function patchManifest(platform, servicePort) {
  const manifest = readFileSync(manifestPath, 'utf8')
    .replace(/^platform\s*=.*$/m, `platform              = ${platform}`)
    .replace(/^service_port\s*=.*$/m, `service_port          = ${servicePort}`);
  writeFileSync(manifestPath, manifest);
}

function patchUiConfig(servicePort) {
  const config = JSON.parse(readFileSync(uiConfigPath, 'utf8'));
  config['.url']['motrix.fnos.main'].port = servicePort;
  writeFileSync(uiConfigPath, JSON.stringify(config, null, 2) + '\n');
}

function ensureFnpack(env) {
  const direct = readOption('--fnpack');
  if (direct) return direct;
  if (which('fnpack', env)) return 'fnpack';

  const version = '1.2.1';
  const hostOs = process.platform === 'darwin' ? 'darwin' : 'linux';
  const hostArch = process.arch === 'arm64' ? 'arm64' : 'amd64';
  const cacheDir = path.join(os.tmpdir(), 'motrix-fnos-fnpack');
  const binary = path.join(cacheDir, `fnpack-${version}-${hostOs}-${hostArch}`);
  mkdirSync(cacheDir, { recursive: true });
  if (!existsSync(binary)) {
    const url = `https://static2.fnnas.com/fnpack/fnpack-${version}-${hostOs}-${hostArch}`;
    run('curl', ['-fsSL', url, '-o', binary], env);
    chmodSync(binary, 0o755);
  }
  return binary;
}

function moveOutputFile() {
  const manifest = parseManifest(readFileSync(manifestPath, 'utf8'));
  const source = path.join(packagingRoot, `${manifest.appname}.fpk`);
  if (!existsSync(source)) {
    fail(`fnpack 未生成预期产物：${source}`);
  }
  mkdirSync(outputDir, { recursive: true });
  resetDir(outputDir);
  const target = path.join(outputDir, `${manifest.appname}_${manifest.version}_${platform}.fpk`);
  copyFileSync(source, target);
  console.log(`FPK 已输出到 ${target}`);
}

function parseManifest(content) {
  return Object.fromEntries(
    content
      .split(/\r?\n/)
      .map((line) => line.match(/^([^#=]+?)\s*=\s*(.+)$/))
      .filter(Boolean)
      .map(([, key, value]) => [key.trim(), value.trim()])
  );
}

function resetDir(dir) {
  for (const entry of readdirSync(dir)) {
    rmSync(path.join(dir, entry), { recursive: true, force: true });
  }
}

function readOption(name) {
  const index = process.argv.indexOf(name);
  if (index === -1) return undefined;
  return process.argv[index + 1];
}

function run(command, args, env, cwd = repoRoot) {
  const result = spawnSync(command, args, { cwd, env, stdio: 'inherit' });
  if (result.status !== 0) {
    process.exit(result.status ?? 1);
  }
}

function which(command, env) {
  const result = spawnSync('sh', ['-lc', `command -v ${command}`], { cwd: repoRoot, env, encoding: 'utf8' });
  return result.status === 0 ? result.stdout.trim() : null;
}

function fail(message) {
  console.error(message);
  process.exit(1);
}
