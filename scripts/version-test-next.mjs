#!/usr/bin/env node
import process from 'node:process';
import { nextTestVersion, readProjectVersions, setProjectVersion } from './version-utils.mjs';

try {
  const currentVersions = readProjectVersions();
  const currentVersion = currentVersions.packageJson;
  const nextVersion = nextTestVersion(currentVersion);

  setProjectVersion(nextVersion);

  const updatedVersions = readProjectVersions();
  console.log(`测试版本已从 ${currentVersion} 迭代为 ${nextVersion}`);
  for (const [source, value] of Object.entries(updatedVersions)) {
    console.log(`- ${source}: ${value}`);
  }
} catch (error) {
  console.error(error instanceof Error ? error.message : String(error));
  process.exit(1);
}
