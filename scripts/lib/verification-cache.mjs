import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { existsSync, lstatSync, mkdirSync, readFileSync, readdirSync, readlinkSync, rmSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { verificationGroups, verificationGroupsForFile } from './verification-inputs.mjs';

const cachePath = '.cache/verification.json';

export function verificationFingerprint() {
  try {
    const files = execFileSync('git', ['ls-files', '--cached', '--others', '--exclude-standard', '-z'],
      { encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] }).split('\0').filter((file) => file && existsSync(file));
    const grouped = Object.fromEntries(verificationGroups.map((group) => [group, []]));
    for (const file of new Set(files)) {
      for (const group of verificationGroupsForFile(file)) grouped[group].push(file);
    }
    const cargoHome = process.env.CARGO_HOME ?? path.join(os.homedir(), '.cargo');
    grouped.rust.push(path.join(cargoHome, 'config'), path.join(cargoHome, 'config.toml'));
    for (const group of ['scripts', 'frontend']) grouped[group].push('node_modules/.modules.yaml', 'node_modules/.pnpm/lock.yaml');
    grouped.frontend.push('.env', '.env.local', '.env.production', '.env.production.local');
    const packageManager = process.env.npm_config_user_agent?.startsWith('pnpm') ? 'pnpm' : 'npm';
    const tools = Object.fromEntries(['rustc', 'cargo', packageManager].map((command) => [command,
      execFileSync(command, [command === 'rustc' ? '-vV' : '--version'],
        { encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] })]));
    const common = [process.platform, process.arch, process.version, process.execPath, process.env.NODE_OPTIONS];
    const environment = Object.entries(process.env).sort(([a], [b]) => a.localeCompare(b));
    const contexts = {
      scripts: [tools, /^(?:CARGO|RUST|VITE_|CC$|CXX$|AR$|LD$)/],
      rust: [[tools.rustc, tools.cargo], /^(?:CARGO|RUST|CC$|CXX$|AR$|LD$)/],
      frontend: [tools[packageManager], /^VITE_/],
    };
    return Object.fromEntries(verificationGroups.map((group) => {
      const [versions, pattern] = contexts[group];
      return [group, hashFiles([...new Set(grouped[group])].sort(),
        [common, versions, environment.filter(([key]) => key !== 'RUST_LOG' && pattern.test(key))])];
    }));
  } catch {
    return null;
  }
}

export function hasVerifiedBuild(inputs, group) {
  if (!inputs?.[group] || process.env.CI) return false;
  try {
    const record = readCache()[group];
    if (record?.inputs !== inputs[group]) return false;
    if (group !== 'frontend') return true;
    const web = webFingerprint();
    return web !== null && record.web === web;
  } catch {
    return false;
  }
}

export function clearVerificationCache(groups) {
  if (groups.length === 0) return;
  const record = readCache();
  for (const group of groups) delete record[group];
  writeCache(record);
}

export function saveVerification(inputs, groups) {
  if (!inputs || groups.length === 0) return;
  const record = readCache();
  for (const group of groups) {
    const web = group === 'frontend' ? webFingerprint() : undefined;
    if (group === 'frontend' && web === null) continue;
    record[group] = { inputs: inputs[group], ...(group === 'frontend' ? { web } : {}) };
  }
  writeCache(record);
}

function readCache() {
  try {
    const record = JSON.parse(readFileSync(cachePath, 'utf8'));
    return record.version === 2 && record.groups && typeof record.groups === 'object' && !Array.isArray(record.groups) ? record.groups : {};
  } catch {
    return {};
  }
}

function writeCache(groups) {
  if (Object.keys(groups).length === 0) {
    rmSync(cachePath, { force: true });
    return;
  }
  mkdirSync(path.dirname(cachePath), { recursive: true });
  writeFileSync(cachePath, `${JSON.stringify({ version: 2, groups })}\n`);
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
