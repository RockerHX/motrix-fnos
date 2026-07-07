#!/usr/bin/env node
import process from 'node:process';
import { assertReleaseVersion, findVersionMismatches, readProjectVersions } from './version-utils.mjs';

try {
  const versions = readProjectVersions();
  assertReleaseVersion(versions.packageJson);

  const mismatches = findVersionMismatches(versions);
  if (mismatches.length > 0) {
    console.error('版本号不一致：');
    for (const mismatch of mismatches) {
      console.error(`- ${mismatch.source}: ${mismatch.version}，期望 ${mismatch.expected}`);
    }
    process.exit(1);
  }

  console.log(`版本检查通过：${versions.packageJson}`);
} catch (error) {
  console.error(error instanceof Error ? error.message : String(error));
  process.exit(1);
}
