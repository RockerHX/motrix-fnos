import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';

const workflow = readFileSync('.github/workflows/prune-releases.yml', 'utf8');
const script = workflow.split('        run: |\n')[1].split('\n').map((line) => line.replace(/^          /, '')).join('\n');
// macOS ships Bash 3; the workflow's Ubuntu runner supplies the real mapfile builtin.
const mapfileCompatibility = `
if (( BASH_VERSINFO[0] < 4 )); then
  mapfile() {
    # Bash 3 treats empty arrays as unset under nounset; blank records are filtered by the workflow.
    local array_name="$2" line first=1
    eval "$array_name=('')"
    while IFS= read -r line; do
      if (( first )); then eval "$array_name=()"; first=0; fi
      eval "$array_name+=(\\"\\$line\\")"
    done
  }
fi
`;

function runRetention({ releases, tags = releases, dryRun = true, confirmation = '', failList = '' }) {
  const root = mkdtempSync(path.join(os.tmpdir(), 'motrix-release-retention-'));
  try {
    const bin = path.join(root, 'bin');
    const stateFile = path.join(root, 'state.json');
    const summary = path.join(root, 'summary');
    mkdirSync(bin);
    writeFileSync(stateFile, JSON.stringify({ releases, tags, actions: [] }));
    const stub = `#!${process.execPath}
const fs = require('node:fs');
const command = require('node:path').basename(process.argv[1]);
const args = process.argv.slice(2);
const file = process.env.FIXTURE_STATE;
const state = JSON.parse(fs.readFileSync(file, 'utf8'));
if (command === 'git' && args[0] === 'fetch') process.exit(0);
if (args[0] === 'api' || args[0] === 'ls-remote') {
  if (process.env.FAIL_LIST === command) process.exit(19);
  console.log(command === 'gh' ? state.releases.join('\\n') : state.tags.map(tag => 'hash\\trefs/tags/' + tag).join('\\n'));
} else if (command === 'gh' && args[0] === 'release' && args[1] === 'delete') {
  state.releases = state.releases.filter(tag => tag !== args[2]);
  state.actions.push('release ' + args[2]);
} else if (command === 'git' && args[0] === 'push') {
  const tag = args[2].replace(':refs/tags/', '');
  state.tags = state.tags.filter(value => value !== tag);
  state.actions.push('tag ' + tag);
} else { throw new Error('Unexpected fixture command: ' + command + ' ' + args.join(' ')); }
fs.writeFileSync(file, JSON.stringify(state));
`;
    for (const command of ['gh', 'git']) {
      writeFileSync(path.join(bin, command), stub, { mode: 0o755 });
    }
    const result = spawnSync('/bin/bash', ['-c', `${mapfileCompatibility}\n${script}`], {
      env: { ...process.env, PATH: `${bin}${path.delimiter}${process.env.PATH}`, FIXTURE_STATE: stateFile,
        FAIL_LIST: failList, DRY_RUN: String(dryRun), CONFIRMATION: confirmation,
        GITHUB_REPOSITORY: 'fixture/fixture', GITHUB_STEP_SUMMARY: summary },
      encoding: 'utf8', timeout: 10_000,
    });
    return { ...result, state: JSON.parse(readFileSync(stateFile, 'utf8')) };
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

const versions = ['v1.9.7', 'v1.9.8', 'v2.9.0', 'v2.10.0'];

test('Release 清理预览保留旧引擎恢复包且不执行删除', () => {
  const result = runRetention({ releases: versions });
  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /保留：`v1\.9\.8`/);
  assert.deepEqual(result.state.actions, []);
});

test('Release 清理只删除非受保护版本，恢复版本与最新两个版本保持一致', () => {
  for (const releases of [versions, ['v1.9.6', 'v1.9.7', 'v1.9.8']]) {
    const result = runRetention({ releases, dryRun: false, confirmation: 'DELETE OLD RELEASES AND TAGS' });
    assert.equal(result.status, 0, result.stderr);
    const removed = releases[0];
    assert.deepEqual(result.state.actions, [`release ${removed}`, `tag ${removed}`]);
    assert.deepEqual(result.state.releases, releases.slice(1));
    assert.deepEqual(result.state.tags, releases.slice(1));
  }
});

test('恢复 Release/tag 缺失、查询失败或确认不符时拒绝删除', () => {
  const withoutRecovery = versions.filter((tag) => tag !== 'v1.9.8');
  for (const fixture of [
    { releases: withoutRecovery, tags: versions },
    { releases: versions, tags: withoutRecovery },
    { releases: versions, failList: 'gh' },
    { releases: versions, failList: 'git' },
    { releases: versions, confirmation: 'wrong' },
  ]) {
    const result = runRetention({ dryRun: false, confirmation: 'DELETE OLD RELEASES AND TAGS', ...fixture });
    assert.notEqual(result.status, 0, `${JSON.stringify(fixture)}\n${result.stdout}\n${result.stderr}`);
    assert.deepEqual(result.state.actions, []);
  }
});
