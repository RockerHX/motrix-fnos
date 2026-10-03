#!/usr/bin/env node
import { spawnSync } from "node:child_process";
import process from "node:process";

for (const filter of ["tasks::files", "runtime::file_cleanup"]) {
  const result = spawnSync(
    "cargo",
    ["test", "--manifest-path", "server/Cargo.toml", "--lib", filter, "--", "--nocapture"],
    { cwd: process.cwd(), env: process.env, stdio: "inherit" },
  );
  if (result.status !== 0) process.exit(result.status ?? 1);
}

console.log("\n文件系统故障测试通过。");
