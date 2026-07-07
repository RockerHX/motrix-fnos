#!/usr/bin/env node
import { execFileSync, spawnSync } from 'node:child_process';
import { readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import process from 'node:process';
import { assertReleaseVersion, readProjectVersions, repoRoot, setProjectVersion } from './version-utils.mjs';

const options = parseArgs(process.argv.slice(2));

try {
  assertReleaseVersion(options.version);

  const currentVersion = readProjectVersions().packageJson;
  if (compareReleaseVersions(options.version, currentVersion) <= 0) {
    fail(`目标版本 ${options.version} 必须大于当前版本 ${currentVersion}`);
  }

  const releaseTag = `v${options.version}`;
  if (tagExists(releaseTag)) {
    fail(`Tag 已存在：${releaseTag}`);
  }

  const statusBefore = gitStatus();
  if (statusBefore.length > 0) {
    if (!options.dryRun) {
      fail(`工作区不干净，拒绝继续：\n${formatStatus(statusBefore)}`);
    }
    console.warn(`工作区不干净，dry-run 仅预览，不会改文件：\n${formatStatus(statusBefore)}`);
  }

  const baseRef = options.from ?? latestReleaseTag();
  const commits = readCommits(baseRef);
  if (commits.length === 0) {
    fail(`${baseRef}..HEAD 没有可用于生成 CHANGELOG 的提交`);
  }

  const existingChangelog = readExistingChangelog(options.version);
  const changelogBody = existingChangelog?.body ?? generateChangelogBody(options.version, commits);
  const changelogSection = existingChangelog?.section ?? renderChangelogSection(options.version, changelogBody);
  const changelogSource = existingChangelog ? 'CHANGELOG.md 已有条目' : 'commit log 生成';
  printPlan({ currentVersion, targetVersion: options.version, baseRef, releaseTag, commits, changelogSection, changelogSource, dryRun: options.dryRun });

  if (options.dryRun) {
    process.exit(0);
  }

  setProjectVersion(options.version);
  if (existingChangelog) {
    console.log(`复用 CHANGELOG.md 中已有的 ${options.version} 条目。`);
  } else {
    updateChangelog(changelogSection, options.version);
  }

  if (!options.noVerify) {
    run('pnpm', ['run', 'verify']);
  } else {
    console.warn('已跳过本地 verify。');
  }

  const statusAfterVerify = gitStatus();
  assertOnlyExpectedReleaseChanges(statusAfterVerify);

  if (!options.noCommit) {
    stageExpectedReleaseFiles();
    run('git', ['commit', '-m', `chore: 发布 ${options.version} 版本`, '-m', changelogBody.trim()]);
  } else {
    console.warn('已跳过 release commit。');
  }

  if (!options.noTag) {
    run('git', ['tag', releaseTag]);
  } else {
    console.warn('已跳过 release tag。');
  }

  console.log(`\n本地发版准备完成：${releaseTag}`);
  console.log('下一步：');
  console.log('  git push');
  console.log(`  git push origin ${releaseTag}`);
} catch (error) {
  console.error(error instanceof Error ? error.message : String(error));
  process.exit(1);
}

function parseArgs(args) {
  const version = args.find((arg) => !arg.startsWith('-'));
  if (!version) {
    console.error('用法：pnpm run release:prepare <x.y.z> [--dry-run] [--from <tag>] [--no-verify] [--no-commit] [--no-tag]');
    process.exit(1);
  }

  return {
    version,
    dryRun: args.includes('--dry-run'),
    noVerify: args.includes('--no-verify'),
    noCommit: args.includes('--no-commit'),
    noTag: args.includes('--no-tag'),
    from: readOption(args, '--from'),
  };
}

function readOption(args, name) {
  const index = args.indexOf(name);
  if (index === -1) return undefined;
  const value = args[index + 1];
  if (!value || value.startsWith('-')) {
    fail(`缺少 ${name} 参数值`);
  }
  return value;
}

function latestReleaseTag() {
  const tag = git(['describe', '--tags', '--abbrev=0', '--match', 'v[0-9]*']);
  if (!tag) {
    fail('未找到历史版本 tag，无法确定 CHANGELOG 起点');
  }
  return tag;
}

function tagExists(tag) {
  const result = spawnSync('git', ['rev-parse', '--verify', '--quiet', `refs/tags/${tag}`], { cwd: repoRoot });
  return result.status === 0;
}

function readCommits(baseRef) {
  const output = git(['log', `${baseRef}..HEAD`, '--no-merges', '--pretty=format:%h%x09%s']);
  return output
    .split('\n')
    .map((line) => line.trim())
    .filter(Boolean)
    .map((line) => {
      const [hash, ...subjectParts] = line.split('\t');
      return { hash, subject: subjectParts.join('\t').trim() };
    });
}

function generateChangelogBody(version, commits) {
  console.warn(`CHANGELOG.md 未包含 ${version} 条目，使用 commit log 生成确定性 CHANGELOG 草稿。`);
  return fallbackChangelog(commits);
}

function fallbackChangelog(commits) {
  const groups = new Map([
    ['新增', []],
    ['修复', []],
    ['文档', []],
    ['改进', []],
  ]);

  for (const commit of commits) {
    if (isReleaseNoiseCommit(commit.subject)) {
      continue;
    }
    const subject = cleanupCommitSubject(commit.subject);
    const group = classifyCommit(commit.subject);
    groups.get(group).push(subject);
  }

  const content = [...groups.entries()]
    .filter(([, items]) => items.length > 0)
    .map(([title, items]) => `### ${title}\n\n${items.map((item) => `- ${item}`).join('\n')}`)
    .join('\n\n');
  return content || '### 改进\n\n- 整理内部实现与发布准备。';
}

function isReleaseNoiseCommit(subject) {
  return (
    /^chore:\s*发布\s+\d+\.\d+\.\d+\s+版本/.test(subject)
    || /^发布\s+\d+\.\d+\.\d+\s+版本/.test(subject)
    || /^Update CHANGELOG\.md$/i.test(subject)
  );
}

function classifyCommit(subject) {
  if (/^(feat|新增)(\(.+\))?:/i.test(subject)) return '新增';
  if (/^(fix|修复)(\(.+\))?:/i.test(subject)) return '修复';
  if (/^(docs|文档)(\(.+\))?:/i.test(subject)) return '文档';
  return '改进';
}

function cleanupCommitSubject(subject) {
  return subject
    .replace(/^(feat|fix|docs|chore|ci|build|refactor|perf|test)(\(.+\))?:\s*/i, '')
    .trim();
}

function renderChangelogSection(version, body) {
  return `## ${version} - ${todayInShanghai()}\n\n${body.trim()}\n`;
}

function readExistingChangelog(version) {
  const changelogPath = path.join(repoRoot, 'CHANGELOG.md');
  const content = readFileSync(changelogPath, 'utf8');
  const pattern = new RegExp(`(^##\\s+${escapeRegExp(version)}\\b[^\\n]*\\n[\\s\\S]*?)(?=^##\\s+|(?![\\s\\S]))`, 'm');
  const match = content.match(pattern);
  if (!match) {
    return null;
  }

  const section = match[1].trimEnd();
  const body = section.replace(/^##[^\n]*\n*/, '').trim();
  if (!body) {
    fail(`CHANGELOG.md 中 ${version} 条目为空`);
  }
  return { section: `${section}\n`, body };
}

function updateChangelog(section, version) {
  const changelogPath = path.join(repoRoot, 'CHANGELOG.md');
  const content = readFileSync(changelogPath, 'utf8');
  if (new RegExp(`^##\\s+${escapeRegExp(version)}\\b`, 'm').test(content)) {
    fail(`CHANGELOG.md 已存在 ${version} 条目`);
  }

  const marker = '# Changelog\n';
  if (!content.startsWith(marker)) {
    fail('CHANGELOG.md 缺少 "# Changelog" 标题');
  }

  writeFileSync(changelogPath, `${marker}\n${section}\n${content.slice(marker.length).trimStart()}`);
}

function assertOnlyExpectedReleaseChanges(statusEntries) {
  const allowedPaths = new Set([
    'CHANGELOG.md',
    'package.json',
    'packaging/fnos/app/ui/config',
    'packaging/fnos/manifest.template',
    'server/Cargo.lock',
    'server/Cargo.toml',
  ]);

  const unexpected = statusEntries.filter((entry) => !allowedPaths.has(entry.path));
  if (unexpected.length > 0) {
    fail(`verify 后出现非 release 预期改动，拒绝自动提交：\n${formatStatus(unexpected)}`);
  }
}

function stageExpectedReleaseFiles() {
  run('git', [
    'add',
    'CHANGELOG.md',
    'package.json',
    'packaging/fnos/app/ui/config',
    'packaging/fnos/manifest.template',
    'server/Cargo.lock',
    'server/Cargo.toml',
  ]);
}

function printPlan({ currentVersion, targetVersion, baseRef, releaseTag, commits, changelogSection, changelogSource, dryRun }) {
  console.log(`\nRelease prepare ${dryRun ? 'dry-run ' : ''}计划`);
  console.log(`- 当前版本：${currentVersion}`);
  console.log(`- 目标版本：${targetVersion}`);
  console.log(`- 起点 tag：${baseRef}`);
  console.log(`- 目标 tag：${releaseTag}`);
  console.log(`- commit 数：${commits.length}`);
  console.log(`- CHANGELOG：${changelogSource}`);
  console.log('\nCHANGELOG 草稿：\n');
  console.log(changelogSection.trim());
  console.log('');
}

function gitStatus() {
  const output = git(['status', '--porcelain']);
  return output
    .split('\n')
    .filter(Boolean)
    .map((line) => ({ status: line.slice(0, 2), path: line.slice(3) }));
}

function formatStatus(entries) {
  return entries.map((entry) => `${entry.status} ${entry.path}`).join('\n');
}

function run(command, args) {
  const result = spawnSync(command, args, { cwd: repoRoot, stdio: 'inherit', env: process.env });
  if (result.status !== 0) {
    process.exit(result.status ?? 1);
  }
}

function git(args) {
  return execFileSync('git', args, { cwd: repoRoot, encoding: 'utf8' }).trim();
}

function compareReleaseVersions(left, right) {
  const leftParts = left.split('.').map(Number);
  const rightParts = right.split('.').map(Number);
  for (let index = 0; index < 3; index += 1) {
    if (leftParts[index] !== rightParts[index]) {
      return leftParts[index] - rightParts[index];
    }
  }
  return 0;
}

function todayInShanghai() {
  const parts = new Intl.DateTimeFormat('en-CA', {
    timeZone: 'Asia/Shanghai',
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
  }).formatToParts(new Date());
  const values = Object.fromEntries(parts.map((part) => [part.type, part.value]));
  return `${values.year}-${values.month}-${values.day}`;
}

function escapeRegExp(value) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

function fail(message) {
  throw new Error(message);
}
