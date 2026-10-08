#!/usr/bin/env node
import { spawnSync } from "node:child_process";
import process from "node:process";

const steps = [
  ["完整源码验证", ["run", "verify"]],
  ["浏览器 E2E 冒烟", ["run", "test:e2e"]],
  ["真实 Aria2 sidecar 集成测试", ["run", "test:aria2"]],
  ["文件系统故障测试", ["run", "test:filesystem"]],
];

for (const [title, args] of steps) {
  console.log(`\n==> ${title}`);
  const result = spawnSync("pnpm", args, {
    cwd: process.cwd(),
    env: process.env,
    stdio: "inherit",
  });
  if (result.status !== 0) process.exit(result.status ?? 1);
}

console.log("\n扩展验证通过。");
