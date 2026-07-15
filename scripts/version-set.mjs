#!/usr/bin/env node
import process from 'node:process';
import { readProjectVersions, setProjectVersion } from './version-utils.mjs';

const version = process.argv[2];

if (!version) {
  console.error('用法：pnpm run version:set <x.y.z[-beta]>');
  process.exit(1);
}

try {
  setProjectVersion(version);
  const versions = readProjectVersions();
  console.log(`版本已同步为 ${version}`);
  for (const [source, value] of Object.entries(versions)) {
    console.log(`- ${source}: ${value}`);
  }
} catch (error) {
  console.error(error instanceof Error ? error.message : String(error));
  process.exit(1);
}
