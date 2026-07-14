#!/usr/bin/env node
import { execFileSync, spawnSync } from 'node:child_process';
import { readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import process from 'node:process';
import { assertReleaseVersion, readProjectVersions, repoRoot, setProjectVersion } from './version-utils.mjs';
import {
  classifyCommit,
  cleanupCommitSubject,
  compareReleaseVersions,
  normalizeGeneratedChangelog,
  validateChangelogBody,
} from './script-utils.mjs';
import {
  buildChangelogPrompt,
  collectReleaseChangeContext,
  readReleaseCommits,
} from './release-changelog-context.mjs';

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

  const existingChangelog = readExistingChangelog(options.version);
  // 自动发版会改版本文件并创建 commit/tag，只允许接管目标版本已有的 CHANGELOG，避免把用户的其他工作区改动混入发布提交。
  const statusBefore = gitStatus();
  if (statusBefore.length > 0) {
    const unexpectedDirtyEntries = initialUnexpectedDirtyEntries(statusBefore, existingChangelog);
    if (unexpectedDirtyEntries.length > 0) {
      fail(`工作区存在非 release prepare 可接管的改动，拒绝继续：\n${formatStatus(unexpectedDirtyEntries)}`);
    }
    console.warn(`工作区已有 CHANGELOG.md 目标版本条目，将由 release prepare 复用：\n${formatStatus(statusBefore)}`);
  }

  const baseRef = options.from ?? latestReleaseTag();
  const commits = readCommits(baseRef);
  if (commits.length === 0) {
    fail(`${baseRef}..HEAD 没有可用于生成 CHANGELOG 的提交`);
  }

  const generatedChangelog = existingChangelog ? null : await generateChangelogBody(options.version, baseRef, commits);
  const changelogBody = existingChangelog?.body ?? generatedChangelog.body;
  const changelogSection = existingChangelog?.section ?? renderChangelogSection(options.version, changelogBody);
  const changelogSource = existingChangelog ? 'CHANGELOG.md 已有条目' : generatedChangelog.source;
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

  // verify 可能生成或改写文件；提交前再次使用发布白名单核对，确保自动提交边界没有被构建副作用扩大。
  const statusAfterVerify = gitStatus();
  assertOnlyExpectedReleaseChanges(statusAfterVerify);

  if (!options.noCommit) {
    // 只暂存固定的版本与 CHANGELOG 文件，不使用 git add -A，防止并发产生的无关文件进入发布 commit。
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
  return readReleaseCommits(repoRoot, baseRef);
}

async function generateChangelogBody(version, baseRef, commits) {
  if (process.env.MOTRIX_RELEASE_CHANGELOG_PROVIDER === 'github-models') {
    const model = process.env.MOTRIX_RELEASE_CHANGELOG_MODEL ?? 'openai/gpt-4o-mini';
    const changeContext = collectReleaseChangeContext({ repoRoot, baseRef, commits });
    const body = await generateChangelogWithGitHubModels({ version, baseRef, changeContext, model });
    return { body, source: `GitHub Models (${model})` };
  }

  console.warn(`CHANGELOG.md 未包含 ${version} 条目，使用 commit log 生成确定性 CHANGELOG 草稿。`);
  return { body: fallbackChangelog(commits), source: 'commit log 生成' };
}

async function generateChangelogWithGitHubModels({ version, baseRef, changeContext, model }) {
  const token = process.env.GITHUB_TOKEN ?? process.env.GH_TOKEN;
  if (!token) {
    throw new Error('缺少 GITHUB_TOKEN/GH_TOKEN');
  }

  const prompt = buildChangelogPrompt({ version, baseRef, changeContext });
  const response = await fetch('https://models.github.ai/inference/chat/completions', {
    method: 'POST',
    headers: {
      Authorization: `Bearer ${token}`,
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({
      model,
      temperature: 0.2,
      messages: [
        {
          role: 'system',
          content: '你是严谨的软件发布说明助手，只根据给定 commit 生成准确、克制、面向用户的中文 CHANGELOG。',
        },
        { role: 'user', content: prompt },
      ],
    }),
  });

  if (!response.ok) {
    const text = await response.text();
    throw new Error(`${response.status} ${text}`);
  }

  const data = await response.json();
  const content = data?.choices?.[0]?.message?.content;
  if (!content) {
    throw new Error('响应缺少 choices[0].message.content');
  }

  return normalizeGeneratedChangelog(content);
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

function renderChangelogSection(version, body) {
  return `## ${version} - ${todayInShanghai()}\n\n${body.trim()}\n`;
}

function readExistingChangelog(version) {
  const changelogPath = path.join(repoRoot, 'CHANGELOG.md');
  const content = readFileSync(changelogPath, 'utf8');
  const lines = content.split(/\r?\n/);
  const start = lines.findIndex((line) => new RegExp(`^##\\s+${escapeRegExp(version)}\\b`).test(line));
  if (start === -1) {
    return null;
  }

  const next = lines.findIndex((line, index) => index > start && /^##\s+/.test(line));
  const sectionLines = lines.slice(start, next === -1 ? undefined : next);
  const section = sectionLines.join('\n').trimEnd();
  const body = section.replace(/^##[^\n]*\n*/, '').trim();
  if (!body) {
    fail(`CHANGELOG.md 中 ${version} 条目为空`);
  }
  validateChangelogBody(body, `CHANGELOG.md 中 ${version} 条目`);
  return { section: `${section}\n`, body };
}

function initialUnexpectedDirtyEntries(statusEntries, existingChangelog) {
  const allowedPaths = existingChangelog ? new Set(['CHANGELOG.md']) : new Set();
  return statusEntries.filter((entry) => !allowedPaths.has(entry.path));
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
  const output = execFileSync('git', ['status', '--porcelain'], { cwd: repoRoot, encoding: 'utf8' });
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
