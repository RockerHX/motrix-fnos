#!/usr/bin/env node
import { chmodSync, copyFileSync, cpSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import os from 'node:os';
import path from 'node:path';
import process from 'node:process';

const repoRoot = process.cwd();
const packagingRoot = path.join(repoRoot, 'packaging', 'fnos');
const manifestTemplatePath = path.join(packagingRoot, 'manifest.template');
const outputDir = path.join(packagingRoot, 'dist');
const buildTarget = readOption('--target') ?? 'x86_64-unknown-linux-gnu';
const platform = buildTarget === 'aarch64-unknown-linux-gnu' ? 'arm' : 'x86';
const stageDir = path.join(packagingRoot, '.stage', platform);
const sidecarTarget = buildTarget;
const prepareOnly = process.argv.includes('--prepare-only');
const keepDist = process.argv.includes('--keep-dist');
const servicePort = readOption('--service-port') ?? '17080';
const env = {
  ...process.env,
  PATH: [path.join(os.homedir(), '.cargo', 'bin'), path.join(os.homedir(), '.local', 'bin'), process.env.PATH ?? ''].filter(Boolean).join(path.delimiter),
};

resetSourceAppDataDir();
run('node', ['scripts/build-server-linux.mjs', '--target', buildTarget], env);
run('node', ['scripts/build-web-ui-fpk.mjs'], env);
run('node', ['scripts/stage-aria2-sidecar.mjs', '--target', sidecarTarget], env);
stageServerBinary(buildTarget);
syncUiIcons();
prepareStageDir();
resetStageAppDataDir(stageDir);
renderManifest(stageDir, platform, servicePort);
patchUiConfig(path.join(stageDir, 'app', 'ui', 'config'), servicePort);
patchPortConfig(path.join(stageDir, 'MotrixFNOS.sc'), servicePort);
removeGitKeepFiles(stageDir);
preflightStageDir(stageDir, platform, servicePort);

if (prepareOnly) {
  console.log(`FPK 预组装完成，目录：${stageDir}`);
  process.exit(0);
}

const fnpack = ensureFnpack(env);
const stageManifest = parseManifest(readFileSync(path.join(stageDir, 'manifest'), 'utf8'));
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
    manifest = upsertManifestField(manifest, 'os_min_version', '1.1.3100');
  } else {
    manifest = upsertManifestField(manifest, 'arch', 'x86_64');
    manifest = upsertManifestField(manifest, 'platform', 'x86');
    manifest = upsertManifestField(manifest, 'os_min_version', '0.9.0');
  }

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
  validateFnosIcon(path.join(dir, 'ICON.PNG'), 64, '包根小图标 ICON.PNG');
  validateFnosIcon(path.join(dir, 'ICON_256.PNG'), 256, '包根大图标 ICON_256.PNG');
  validateFnosIcon(path.join(dir, 'app', 'ui', 'images', 'icon_64.png'), 64, '入口小图标 icon_64.png');
  validateFnosIcon(path.join(dir, 'app', 'ui', 'images', 'icon_256.png'), 256, '入口大图标 icon_256.png');
  validateFnosIcon(path.join(dir, 'app', 'ui', 'dist', 'icon.png'), 128, 'Web 图标 icon.png');

  const manifest = parseManifest(readFileSync(path.join(dir, 'manifest'), 'utf8'));
  const expectedUiDir = manifest.desktop_uidir || 'ui';
  const desktopUiDir = path.join(dir, 'app', expectedUiDir);
  if (!existsSync(desktopUiDir)) {
    fail(`FPK 预检失败，desktop_uidir 对应目录不存在：${desktopUiDir}`);
  }

  if (manifest.service_port !== servicePort) {
    fail(`FPK 预检失败，manifest.service_port=${manifest.service_port} 与预期 ${servicePort} 不一致`);
  }

  if (platform === 'x86') {
    if (manifest.platform !== 'x86') {
      fail(`FPK 预检失败，x86 包 platform 应为 x86，实际为 ${manifest.platform ?? '(missing)'}`);
    }
    if (manifest.arch !== 'x86_64') {
      fail(`FPK 预检失败，x86 包 arch 应为 x86_64，实际为 ${manifest.arch ?? '(missing)'}`);
    }
    if (manifest.os_min_version !== '0.9.0') {
      fail(`FPK 预检失败，x86 包 os_min_version 应为 0.9.0，实际为 ${manifest.os_min_version ?? '(missing)'}`);
    }
  } else {
    if (manifest.platform !== 'arm') {
      fail(`FPK 预检失败，ARM 包 platform 应为 arm，实际为 ${manifest.platform ?? '(missing)'}`);
    }
    if (manifest.arch) {
      fail(`FPK 预检失败，ARM 包不应声明 arch，实际为 ${manifest.arch}`);
    }
    if (manifest.os_min_version !== '1.1.3100') {
      fail(`FPK 预检失败，ARM 包 os_min_version 应为 1.1.3100，实际为 ${manifest.os_min_version ?? '(missing)'}`);
    }
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
    JSON.parse(readFileSync(filePath, 'utf8'));
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    fail(`FPK 预检失败，${label} JSON 格式无效：${filePath}，${message}`);
  }
}

function upsertManifestField(content, key, value) {
  const line = `${key.padEnd(22, ' ')}= ${value}`;
  const pattern = new RegExp(`^${escapeRegExp(key)}\\s*=.*$`, 'm');
  if (pattern.test(content)) {
    return content.replace(pattern, line);
  }

  const sourcePattern = /^source\s*=.*$/m;
  if (sourcePattern.test(content)) {
    return content.replace(sourcePattern, (sourceLine) => `${sourceLine}
${line}`);
  }

  return `${content.trimEnd()}
${line}
`;
}

function removeManifestField(content, key) {
  const pattern = new RegExp(`^${escapeRegExp(key)}\\s*=.*\\r?\\n?`, 'm');
  return content.replace(pattern, '');
}

function escapeRegExp(value) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

function patchUiConfig(uiConfigPath, servicePort) {
  const config = JSON.parse(readFileSync(uiConfigPath, 'utf8'));
  config['.url']['motrix.fnos.main'].port = servicePort;
  writeFileSync(uiConfigPath, JSON.stringify(config, null, 2) + '\n');
}

function patchPortConfig(portConfigPath, servicePort) {
  const port = `${servicePort}/tcp`;
  let config = readFileSync(portConfigPath, 'utf8');
  config = config.replace(/^src\.ports=.*$/m, `src.ports="${port}"`);
  config = config.replace(/^dst\.ports=.*$/m, `dst.ports="${port}"`);
  writeFileSync(portConfigPath, config);
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
  copyFileSync(source, target);
  console.log(`FPK 已输出到 ${target}`);
}

function injectPackageRootFiles(fpkPath, portConfigPath) {
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
