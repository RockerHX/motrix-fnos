#!/usr/bin/env node
import { spawn } from "node:child_process";
import process from "node:process";

const quick = process.argv.includes("--quick");
const keepRustIncremental = process.argv.includes("--keep-rust-incremental");
const packageManager = resolvePackageManager();
const rustEnv = {
  ...process.env,
  RUSTFLAGS: appendRustDenyWarnings(process.env.RUSTFLAGS),
};

const steps = quick
  ? [
      { title: "项目版本一致性检查", command: "node", args: ["scripts/version-check.mjs"] },
      { title: "FPK 进程身份校验测试", command: "sh", args: ["scripts/test-fnos-process-identity.sh"] },
      { title: "Rust 测试（warnings as errors）", command: "cargo", args: ["test", "--manifest-path", "server/Cargo.toml"], env: rustEnv },
      { title: "前端类型检查", command: packageManager, args: ["run", "typecheck"] },
      { title: "前端单元测试", command: packageManager, args: ["run", "test:unit"] },
    ]
  : [
      { title: "项目版本一致性检查", command: "node", args: ["scripts/version-check.mjs"] },
      { title: "FPK 进程身份校验测试", command: "sh", args: ["scripts/test-fnos-process-identity.sh"] },
      { title: "Rust 测试（warnings as errors）", command: "cargo", args: ["test", "--manifest-path", "server/Cargo.toml"], env: rustEnv },
      { title: "Rust 编译（warnings as errors）", command: "cargo", args: ["build", "--manifest-path", "server/Cargo.toml"], env: rustEnv },
      { title: "前端类型检查", command: packageManager, args: ["run", "typecheck"] },
      { title: "前端单元测试", command: packageManager, args: ["run", "test:unit"] },
      { title: "前端构建", command: packageManager, args: ["run", "build"] },
    ];

try {
  for (const step of steps) {
    await runStep(step);
  }

  console.log(quick ? "快速验证通过。" : "完整验证通过。");
} finally {
  if (!keepRustIncremental) {
    await pruneRustIncrementalCache();
  }
}

function runStep(step) {
  console.log(`\n==> ${step.title}`);
  return new Promise((resolve, reject) => {
    const child = spawn(resolveCommand(step.command), step.args, {
      cwd: process.cwd(),
      env: step.env ?? process.env,
      stdio: "inherit",
      shell: false,
    });

    child.on("error", reject);
    child.on("exit", (code, signal) => {
      if (code === 0) {
        resolve();
        return;
      }

      reject(new Error(`${step.title} 失败：${signal ?? code}`));
    });
  });
}

async function pruneRustIncrementalCache() {
  console.log("\n==> 清理 Rust incremental 缓存");
  try {
    await runStep({
      title: "Rust incremental 缓存清理",
      command: "node",
      args: ["scripts/clean-rust-target.mjs", "--incremental"],
    });
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    console.warn(`Rust incremental 缓存清理失败，已忽略：${message}`);
  }
}

function resolvePackageManager() {
  const userAgent = process.env.npm_config_user_agent ?? "";
  if (userAgent.startsWith("pnpm")) {
    return "pnpm";
  }
  if (userAgent.startsWith("yarn")) {
    return "yarn";
  }
  return "npm";
}

function resolveCommand(command) {
  if (process.platform !== "win32") {
    return command;
  }

  if (command === "npm" || command === "pnpm" || command === "yarn") {
    return `${command}.cmd`;
  }
  return command;
}

function appendRustDenyWarnings(value = "") {
  const flags = value.split(/\s+/).filter(Boolean);
  if (!flags.includes("-D") || !flags.includes("warnings")) {
    flags.push("-D", "warnings");
  }
  return flags.join(" ");
}
