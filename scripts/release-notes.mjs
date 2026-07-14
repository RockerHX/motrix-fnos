#!/usr/bin/env node
import { readFileSync } from 'node:fs';
import path from 'node:path';
import process from 'node:process';
import { validateChangelogBody } from './script-utils.mjs';

const version = process.argv[2];
const bodyOnly = process.argv.includes('--body');
const repoRoot = process.cwd();

if (!version) {
  console.error('用法：node scripts/release-notes.mjs <x.y.z> [--body]');
  process.exit(1);
}

try {
  const section = readChangelogSection(version);
  if (bodyOnly) {
    console.log(section.body);
  } else {
    console.log(section.section);
  }
} catch (error) {
  console.error(error instanceof Error ? error.message : String(error));
  process.exit(1);
}

function readChangelogSection(targetVersion) {
  const changelogPath = path.join(repoRoot, 'CHANGELOG.md');
  const content = readFileSync(changelogPath, 'utf8');
  const lines = content.split(/\r?\n/);
  const start = lines.findIndex((line) => new RegExp(`^##\\s+${escapeRegExp(targetVersion)}\\b`).test(line));
  if (start === -1) {
    throw new Error(`CHANGELOG.md 缺少 ${targetVersion} 条目`);
  }

  const next = lines.findIndex((line, index) => index > start && /^##\s+/.test(line));
  const sectionLines = lines.slice(start, next === -1 ? undefined : next);
  const section = sectionLines.join('\n').trimEnd();
  const body = section.replace(/^##[^\n]*\n*/, '').trim();
  if (!body) {
    throw new Error(`CHANGELOG.md 中 ${targetVersion} 条目为空`);
  }
  validateChangelogBody(body, `CHANGELOG.md 中 ${targetVersion} 条目`);

  return { section, body };
}

function escapeRegExp(value) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}
