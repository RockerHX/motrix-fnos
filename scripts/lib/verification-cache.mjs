import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { existsSync, lstatSync, mkdirSync, readFileSync, readdirSync, readlinkSync, rmSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';

const cachePath = '.cache/verification.json';

export function verificationFingerprint() {
  try {
    const files = execFileSync('git', ['ls-files', '--cached', '--others', '--exclude-standard', '-z'],
      { encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] }).split('\0').filter((file) => file && existsSync(file));
    const cargoHome = process.env.CARGO_HOME ?? path.join(os.homedir(), '.cargo');
    files.push('node_modules/.modules.yaml', 'node_modules/.pnpm/lock.yaml',
      '.env', '.env.local', '.env.production', '.env.production.local',
      path.join(cargoHome, 'config'), path.join(cargoHome, 'config.toml'));
    const packageManager = process.env.npm_config_user_agent?.startsWith('pnpm') ? 'pnpm' : 'npm';
    const tools = ['rustc', 'cargo', packageManager].map((command) =>
      execFileSync(command, [command === 'rustc' ? '-vV' : '--version'],
        { encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] }));
    const environment = Object.entries(process.env)
      .filter(([key]) => /^(?:CARGO|RUST|VITE_|NODE_OPTIONS$|CC$|CXX$|AR$|LD$)/.test(key))
      .sort(([a], [b]) => a.localeCompare(b));
    return hashFiles([...new Set(files)].sort(),
      [process.platform, process.arch, process.version, process.execPath, tools, environment]);
  } catch {
    return null;
  }
}

export function hasVerifiedBuild(inputs) {
  if (!inputs || process.env.CI) return false;
  try {
    const record = JSON.parse(readFileSync(cachePath, 'utf8'));
    const web = webFingerprint();
    return record.inputs === inputs && web !== null && record.web === web;
  } catch {
    return false;
  }
}

export function clearVerificationCache() {
  rmSync(cachePath, { force: true });
}

export function saveVerification(inputs) {
  const web = webFingerprint();
  if (!inputs || web === null) return;
  mkdirSync(path.dirname(cachePath), { recursive: true });
  writeFileSync(cachePath, `${JSON.stringify({ inputs, web })}\n`);
}

function webFingerprint() {
  try {
    if (!existsSync('dist/index.html')) return null;
    const files = [];
    const walk = (dir) => {
      for (const entry of readdirSync(dir, { withFileTypes: true })) {
        const file = path.join(dir, entry.name);
        if (entry.isDirectory()) walk(file);
        else files.push(file);
      }
    };
    walk('dist');
    return hashFiles(files.sort());
  } catch {
    return null;
  }
}

function hashFiles(files, context = []) {
  const hash = createHash('sha256').update(JSON.stringify(context));
  for (const file of files) {
    let stat;
    try {
      stat = lstatSync(file);
    } catch (error) {
      if (error.code !== 'ENOENT') throw error;
      hash.update(JSON.stringify([file, 'missing']));
      continue;
    }
    const content = readFileSync(file);
    hash.update(JSON.stringify([file, stat.mode, stat.isSymbolicLink() ? readlinkSync(file) : null, content.length]));
    hash.update(content);
  }
  return hash.digest('hex');
}
