#!/usr/bin/env node
import { performance } from "node:perf_hooks";
import process from "node:process";
import { cargoProgressDetail, runCommandWithProgress } from "../lib/command-progress.mjs";

const quick = process.argv.includes("--quick");
const packageManager = resolvePackageManager();

const steps = quick
  ? [
      { title: "项目版本一致性检查", command: "node", args: ["scripts/version/version-check.mjs"], detail: "核对所有版本来源" },
      { title: "Rust 格式检查", command: "cargo", args: ["fmt", "--manifest-path", "server/Cargo.toml", "--all", "--", "--check"], detail: "检查 Rust 源码格式" },
    ]
  : [
      { title: "项目版本一致性检查", command: "node", args: ["scripts/version/version-check.mjs"], detail: "核对所有版本来源" },
      { title: "Rust 格式检查", command: "cargo", args: ["fmt", "--manifest-path", "server/Cargo.toml", "--all", "--", "--check"], detail: "检查 Rust 源码格式" },
      { title: "构建与发布脚本测试", command: packageManager, args: ["run", "test:scripts"], detail: "收集 Node.js 测试" },
      { title: "FPK 进程身份校验测试", command: "sh", args: ["scripts/verify/test-fnos-process-identity.sh"], detail: "运行进程身份 Shell 场景" },
      { title: "FPK 启动孤儿进程对账测试", command: "sh", args: ["scripts/verify/test-fnos-startup-reconcile.sh"], detail: "运行启动残留进程安全对账场景" },
      { title: "FPK 停止卸载收敛测试", command: "sh", args: ["scripts/verify/test-fnos-lifecycle-stop.sh"], detail: "运行信号升级与卸载失败关闭场景" },
      { title: "FPK 服务就绪脚本测试", command: "sh", args: ["scripts/verify/test-fnos-readiness.sh"], detail: "运行服务就绪 Shell 场景" },
      { title: "Rust 测试（warnings as errors）", command: "node", args: ["scripts/verify/run-rust-tests.mjs"], detail: "收集 Rust 测试目标" },
      { title: "Rust 编译（warnings as errors）", command: "cargo", args: ["build", "--manifest-path", "server/Cargo.toml"], detail: "准备 Cargo 编译", activity: cargoProgressDetail },
      { title: "前端单元测试", command: packageManager, args: ["run", "test:unit"], detail: "收集 Vitest 测试文件" },
      { title: "前端类型检查与构建", command: packageManager, args: ["run", "build"], detail: "准备前端类型检查" },
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
  return runCommandWithProgress(resolveCommand(step.command), step.args, {
    title: step.title,
    cwd: process.cwd(),
    env: step.env ?? process.env,
    initialDetail: step.detail,
    activity: step.activity,
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

function formatDuration(milliseconds) {
  return `${(milliseconds / 1000).toFixed(2)}s`;
}
