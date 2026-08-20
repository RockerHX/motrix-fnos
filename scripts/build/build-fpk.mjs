#!/usr/bin/env node
import { chmodSync, copyFileSync, cpSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import os from 'node:os';
import path from 'node:path';
import process from 'node:process';
import { reportCommandProgress } from '../lib/command-progress.mjs';
import {
  parseManifest,
  platformForTarget,
  removeManifestField,
  resolveFpkEntryId,
  upsertManifestField,
  validateFpkAppIdentity,
  validateFpkArtifactName,
  validateFpkPortIsolation,
  validateFpkRuntimeEnvScript,
  validateFpkRuntimeDataEntries,
} from '../lib/script-utils.mjs';

const repoRoot = process.cwd();
const packagingRoot = path.join(repoRoot, 'packaging', 'fnos');
const manifestTemplatePath = path.join(packagingRoot, 'manifest.template');
const outputDir = path.join(packagingRoot, 'dist');
const buildTarget = readOption('--target') ?? 'x86_64-unknown-linux-gnu';
const platform = platformForTarget(buildTarget);
const stageDir = path.join(packagingRoot, '.stage', platform);
const sidecarTarget = buildTarget;
const prepareOnly = process.argv.includes('--prepare-only');
const keepDist = process.argv.includes('--keep-dist');
const reuseWebUi = process.argv.includes('--reuse-web-ui');
const servicePort = readOption('--service-port') ?? '17080';
const lanJsonRpcPort = '17082';
const minimumFnosVersion = '1.2.0401';
const env = {
  ...process.env,
  PATH: [path.join(os.homedir(), '.cargo', 'bin'), path.join(os.homedir(), '.local', 'bin'), process.env.PATH ?? ''].filter(Boolean).join(path.delimiter),
};

resetSourceAppDataDir();
reportCommandProgress(`准备 Linux server：${buildTarget}`);
run('node', ['scripts/build/build-server-linux.mjs', '--target', buildTarget], env);
if (!reuseWebUi) {
  reportCommandProgress('构建 FPK Web UI');
  run('node', ['scripts/build/build-web-ui-fpk.mjs'], env);
}
reportCommandProgress(`放置 Aria2 Next sidecar：${sidecarTarget}`);
run('node', ['scripts/build/stage-aria2-sidecar.mjs', '--target', sidecarTarget], env);
reportCommandProgress('组装并预检 FPK 文件结构');
stageServerBinary(buildTarget);
syncUiIcons();
prepareStageDir();
resetStageAppDataDir(stageDir);
renderManifest(stageDir, platform, servicePort);
const stageManifest = parseManifest(readFileSync(path.join(stageDir, 'manifest'), 'utf8'));
patchUiPort(path.join(stageDir, 'app', 'ui', 'config'), servicePort, stageManifest.desktop_applaunchname);
patchPortConfig(path.join(stageDir, 'MotrixFNOS.sc'), servicePort, lanJsonRpcPort);
removeGitKeepFiles(stageDir);
preflightStageDir(stageDir, platform, servicePort);

if (prepareOnly) {
  console.log(`FPK 预组装完成，目录：${stageDir}`);
  process.exit(0);
}

const fnpack = ensureFnpack(env);
reportCommandProgress('执行 fnpack 打包');
const stagedPackagePath = path.join(stageDir, `${stageManifest.appname}.fpk`);
rmSync(stagedPackagePath, { force: true });
const buildOutput = runAndCapture(fnpack, ['build', '--directory', stageDir], env, stageDir);
if (buildOutput.includes('Packing failed')) {
  fail('fnpack 输出包含 "Packing failed"，已中止打包流程');
}
moveOutputFile(stageDir);

function removeGitKeepFiles(dir) {
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const target = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      removeGitKeepFiles(target);
    } else if (entry.name === '.gitkeep') {
      rmSync(target, { force: true });
    }
  }
}

function resetSourceAppDataDir() {
  const dataDir = path.join(packagingRoot, 'app', 'data');
  mkdirSync(dataDir, { recursive: true });
  for (const entry of readdirSync(dataDir)) {
    rmSync(path.join(dataDir, entry), { recursive: true, force: true });
  }
}

function resetStageAppDataDir(dir) {
  const dataDir = path.join(dir, 'app', 'data');
  mkdirSync(dataDir, { recursive: true });
  for (const entry of readdirSync(dataDir)) {
    rmSync(path.join(dataDir, entry), { recursive: true, force: true });
  }
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
  rmSync(path.join(imagesDir, 'icon-64.png'), { force: true });
  rmSync(path.join(imagesDir, 'icon-128.png'), { force: true });
  rmSync(path.join(imagesDir, 'icon-256.png'), { force: true });
  rmSync(path.join(imagesDir, 'icon_64.png'), { force: true });
  rmSync(path.join(imagesDir, 'icon_256.png'), { force: true });
  copyFileSync(path.join(packagingRoot, 'ICON.PNG'), path.join(imagesDir, 'icon_64.png'));
  copyFileSync(path.join(packagingRoot, 'ICON_256.PNG'), path.join(imagesDir, 'icon_256.png'));
}

function prepareStageDir() {
  // 源码目录包含模板和本地生成物，不能直接交给 fnpack；每次从白名单源重新生成独立 stage，避免把残留数据打进安装包。
  rmSync(stageDir, { recursive: true, force: true });
  mkdirSync(stageDir, { recursive: true });

  for (const entry of readdirSync(packagingRoot, { withFileTypes: true })) {
    if (entry.name === '.stage' || entry.name === 'dist' || entry.name === 'manifest.template' || entry.name === 'manifest') {
      continue;
    }

    const source = path.join(packagingRoot, entry.name);
    const destination = path.join(stageDir, entry.name);
    cpSync(source, destination, { recursive: true });
  }
}

function renderManifest(dir, platform, servicePort) {
  let manifest = readFileSync(manifestTemplatePath, 'utf8');
  const isArm = platform === 'arm';

  if (isArm) {
    manifest = upsertManifestField(manifest, 'platform', 'arm');
    manifest = removeManifestField(manifest, 'arch');
  } else {
    manifest = upsertManifestField(manifest, 'arch', 'x86_64');
    manifest = upsertManifestField(manifest, 'platform', 'x86');
  }

  manifest = upsertManifestField(manifest, 'os_min_version', minimumFnosVersion);
  manifest = removeManifestField(manifest, 'disable_authorization_path');
  manifest = upsertManifestField(manifest, 'service_port', servicePort);
  writeFileSync(path.join(dir, 'manifest'), manifest);
}

function preflightStageDir(dir, platform, servicePort) {
  const requiredPaths = [
    'manifest',
    'config/privilege',
    'config/resource',
    'ICON.PNG',
    'ICON_256.PNG',
    'app',
    'cmd',
    'cmd/main',
    'cmd/install_init',
    'cmd/install_callback',
    'cmd/upgrade_init',
    'cmd/upgrade_callback',
    'cmd/uninstall_init',
    'cmd/uninstall_callback',
    'wizard',
    'wizard/install',
    'wizard/uninstall',
    'MotrixFNOS.sc',
  ];

  for (const relativePath of requiredPaths) {
    if (!existsSync(path.join(dir, relativePath))) {
      fail(`FPK 预检失败，缺少必需文件：${path.join(dir, relativePath)}`);
    }
  }

  validateJsonFile(path.join(dir, 'wizard', 'install'), '安装向导');
  validateJsonFile(path.join(dir, 'wizard', 'uninstall'), '卸载向导');
  try {
    validateFpkRuntimeDataEntries(readdirSync(path.join(dir, 'app', 'data')));
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    fail(`FPK 预检失败，${message}`);
  }
  validateFnosIcon(path.join(dir, 'ICON.PNG'), 256, '包图标 ICON.PNG');
  validateFnosIcon(path.join(dir, 'ICON_256.PNG'), 256, '包图标 ICON_256.PNG');
  validateFnosIcon(path.join(dir, 'app', 'ui', 'images', 'icon_64.png'), 256, '入口图标 icon_64.png');
  validateFnosIcon(path.join(dir, 'app', 'ui', 'images', 'icon_256.png'), 256, '入口图标 icon_256.png');
  validateFnosIcon(path.join(dir, 'app', 'ui', 'dist', 'icon.png'), 256, 'Web 图标 icon.png');

  const manifest = parseManifest(readFileSync(path.join(dir, 'manifest'), 'utf8'));
  const expectedUiDir = manifest.desktop_uidir || 'ui';
  const desktopUiDir = path.join(dir, 'app', expectedUiDir);
  if (!existsSync(desktopUiDir)) {
    fail(`FPK 预检失败，desktop_uidir 对应目录不存在：${desktopUiDir}`);
  }
  const uiConfig = validateJsonFile(path.join(desktopUiDir, 'config'), '应用入口');
  try {
    validateFpkAppIdentity({
      manifestContent: readFileSync(path.join(dir, 'manifest'), 'utf8'),
      uiConfig,
      expectedAppName: 'motrix',
      expectedEntryId: 'motrix.Application',
    });
    validateFpkPortIsolation({
      manifestContent: readFileSync(path.join(dir, 'manifest'), 'utf8'),
      uiConfig,
      portConfigContent: readFileSync(path.join(dir, 'MotrixFNOS.sc'), 'utf8'),
      resourceContent: readFileSync(path.join(dir, 'config', 'resource'), 'utf8'),
      managementPort: servicePort,
      jsonRpcPort: '17081',
      lanJsonRpcPort,
    });
    validateFpkRuntimeEnvScript(
      readFileSync(path.join(dir, 'cmd', 'common.sh'), 'utf8'),
      '127.0.0.1:17081',
      '0.0.0.0:17082',
    );
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    fail(`FPK 预检失败，三监听器端口隔离无效：${message}`);
  }

  if (platform === 'x86') {
    if (manifest.platform !== 'x86') {
      fail(`FPK 预检失败，x86 包 platform 应为 x86，实际为 ${manifest.platform ?? '(missing)'}`);
    }
    if (manifest.arch !== 'x86_64') {
      fail(`FPK 预检失败，x86 包 arch 应为 x86_64，实际为 ${manifest.arch ?? '(missing)'}`);
    }
  } else {
    if (manifest.platform !== 'arm') {
      fail(`FPK 预检失败，ARM 包 platform 应为 arm，实际为 ${manifest.platform ?? '(missing)'}`);
    }
    if (manifest.arch) {
      fail(`FPK 预检失败，ARM 包不应声明 arch，实际为 ${manifest.arch}`);
    }
  }
  if (manifest.os_min_version !== minimumFnosVersion) {
    fail(`FPK 预检失败，os_min_version 应为 ${minimumFnosVersion}，实际为 ${manifest.os_min_version ?? '(missing)'}`);
  }
}

function validateFnosIcon(filePath, expectedSize, label) {
  if (!existsSync(filePath)) {
    fail(`FPK 预检失败，缺少${label}：${filePath}`);
  }

  const content = readFileSync(filePath);
  const pngSignature = Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);
  if (content.length < 33 || !content.subarray(0, 8).equals(pngSignature)) {
    fail(`FPK 预检失败，${label} 必须是 PNG：${filePath}`);
  }

  const width = content.readUInt32BE(16);
  const height = content.readUInt32BE(20);
  const bitDepth = content.readUInt8(24);
  const colorType = content.readUInt8(25);

  if (width !== expectedSize || height !== expectedSize) {
    fail(`FPK 预检失败，${label} 尺寸应为 ${expectedSize}x${expectedSize}，实际为 ${width}x${height}`);
  }

  if (bitDepth !== 8 || colorType !== 6) {
    fail(`FPK 预检失败，${label} 必须是 8-bit RGBA PNG，实际 bitDepth=${bitDepth} colorType=${colorType}`);
  }
}

function validateJsonFile(filePath, label) {
  try {
    return JSON.parse(readFileSync(filePath, 'utf8'));
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    fail(`FPK 预检失败，${label} JSON 格式无效：${filePath}，${message}`);
  }
}

function patchPortConfig(portConfigPath, servicePort, lanJsonRpcPort) {
  const port = `${servicePort}/tcp,${lanJsonRpcPort}/tcp`;
  let config = readFileSync(portConfigPath, 'utf8');
  config = config.replace(/^src\.ports=.*$/m, `src.ports="${port}"`);
  config = config.replace(/^dst\.ports=.*$/m, `dst.ports="${port}"`);
  writeFileSync(portConfigPath, config);
}

function patchUiPort(uiConfigPath, servicePort, entryId) {
  const config = JSON.parse(readFileSync(uiConfigPath, 'utf8'));
  let resolvedEntryId;
  try {
    resolvedEntryId = resolveFpkEntryId(config, entryId);
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    fail(`无法确定应用入口：${message}`);
  }
  const entry = config['.url']?.[resolvedEntryId];
  if (!entry || typeof entry !== 'object') {
    fail(`应用入口配置缺少 manifest.desktop_applaunchname 对应入口：${resolvedEntryId}`);
  }
  entry.port = servicePort;
  writeFileSync(uiConfigPath, `${JSON.stringify(config, null, 2)}\n`);
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

function moveOutputFile(dir) {
  const manifest = parseManifest(readFileSync(path.join(dir, 'manifest'), 'utf8'));
  const source = path.join(dir, `${manifest.appname}.fpk`);
  if (!existsSync(source)) {
    fail(`fnpack 未生成预期产物：${source}`);
  }
  injectPackageRootFiles(source, path.join(dir, 'MotrixFNOS.sc'));
  mkdirSync(outputDir, { recursive: true });
  if (!keepDist) {
    resetDir(outputDir);
  }
  const target = path.join(outputDir, `${manifest.appname}_${manifest.version}_${platform}.fpk`);
  validateFpkArtifactName(path.basename(target), manifest.version, platform);
  copyFileSync(source, target);
  console.log(`FPK 已输出到 ${target}`);
}

function injectPackageRootFiles(fpkPath, portConfigPath) {
  // fnpack 1.2.1 不会稳定保留 MotrixFNOS.sc 到包根，因此在产物生成后重新封包；升级 fnpack 时需重新验证这一兼容步骤。
  const workDir = mkdtempSync(path.join(os.tmpdir(), 'motrix-fnos-fpk-'));
  try {
    run('tar', ['-xzf', fpkPath, '-C', workDir], env);
    copyFileSync(portConfigPath, path.join(workDir, path.basename(portConfigPath)));
    const entries = readdirSync(workDir);
    run('tar', ['-czf', fpkPath, ...entries], env, workDir);
  } finally {
    rmSync(workDir, { recursive: true, force: true });
  }
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

function runAndCapture(command, args, env, cwd = repoRoot) {
  const result = spawnSync(command, args, { cwd, env, encoding: 'utf8' });
  if (result.stdout) {
    process.stdout.write(result.stdout);
  }
  if (result.stderr) {
    process.stderr.write(result.stderr);
  }
  if (result.status !== 0) {
    process.exit(result.status ?? 1);
  }
  return `${result.stdout ?? ''}${result.stderr ?? ''}`;
}

function which(command, env) {
  const result = spawnSync('sh', ['-lc', `command -v ${command}`], { cwd: repoRoot, env, encoding: 'utf8' });
  return result.status === 0 ? result.stdout.trim() : null;
}

function fail(message) {
  console.error(message);
  process.exit(1);
}
