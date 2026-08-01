#!/usr/bin/env node
import { spawn } from "node:child_process";
import { performance } from "node:perf_hooks";
import process from "node:process";

const quick = process.argv.includes("--quick");
const packageManager = resolvePackageManager();
const rustEnv = {
  ...process.env,
  RUSTFLAGS: appendRustDenyWarnings(process.env.RUSTFLAGS),
};

const steps = quick
  ? [
      { title: "项目版本一致性检查", command: "node", args: ["scripts/version-check.mjs"] },
      { title: "Rust 格式检查", command: "cargo", args: ["fmt", "--manifest-path", "server/Cargo.toml", "--all", "--", "--check"] },
    ]
  : [
      { title: "项目版本一致性检查", command: "node", args: ["scripts/version-check.mjs"] },
      { title: "Rust 格式检查", command: "cargo", args: ["fmt", "--manifest-path", "server/Cargo.toml", "--all", "--", "--check"] },
      { title: "构建与发布脚本测试", command: packageManager, args: ["run", "test:scripts"] },
      { title: "FPK 进程身份校验测试", command: "sh", args: ["scripts/test-fnos-process-identity.sh"] },
      { title: "FPK 服务就绪脚本测试", command: "sh", args: ["scripts/test-fnos-readiness.sh"] },
      { title: "Rust 测试（warnings as errors）", command: "node", args: ["scripts/run-rust-tests.mjs"], env: rustEnv },
      { title: "Rust 编译（warnings as errors）", command: "cargo", args: ["build", "--manifest-path", "server/Cargo.toml"], env: rustEnv },
      { title: "前端单元测试", command: packageManager, args: ["run", "test:unit"] },
      { title: "前端类型检查与构建", command: packageManager, args: ["run", "build"] },
    ];

const verificationStartedAt = performance.now();
for (const step of steps) {
  const stepStartedAt = performance.now();
  await runStep(step);
  console.log(`<== ${step.title}通过（${formatDuration(performance.now() - stepStartedAt)}）`);
}

console.log(`${quick ? "快速验证" : "完整验证"}通过，总耗时 ${formatDuration(performance.now() - verificationStartedAt)}。`);

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

function formatDuration(milliseconds) {
  return `${(milliseconds / 1000).toFixed(2)}s`;
}
