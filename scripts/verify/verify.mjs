#!/usr/bin/env node
import { performance } from "node:perf_hooks";
import process from "node:process";
import { cargoProgressDetail, runCommandWithProgress } from "../lib/command-progress.mjs";
import { clearVerificationCache, hasVerifiedBuild, saveVerification, verificationFingerprint } from "../lib/verification-cache.mjs";
import { verificationGroups } from "../lib/verification-inputs.mjs";

const quick = process.argv.includes("--quick");
const packageManager = resolvePackageManager();
const inputs = !quick && !process.env.CI ? verificationFingerprint() : null;
const reused = new Set(process.argv.includes("--reuse") && !quick
  ? verificationGroups.filter((group) => hasVerifiedBuild(inputs, group)) : []);
const pending = verificationGroups.filter((group) => !reused.has(group));
if (!quick) {
  clearVerificationCache(pending);
  for (const [group, label] of [["scripts", "脚本检查"], ["rust", "后端验证"], ["frontend", "前端验证"]]) {
    console.log(reused.has(group) ? `[复用] ${label}：复用本地已通过的验证。` : `[执行] ${label}`);
  }
}

const steps = quick
  ? [
      { title: "项目版本一致性检查", command: "node", args: ["scripts/version/version-check.mjs"], detail: "核对所有版本来源" },
      { title: "Rust 格式检查", command: "cargo", args: ["fmt", "--manifest-path", "server/Cargo.toml", "--all", "--", "--check"], detail: "检查 Rust 源码格式" },
    ]
  : [
      { title: "项目版本一致性检查", command: "node", args: ["scripts/version/version-check.mjs"], detail: "核对所有版本来源" },
      { group: "rust", title: "Rust 格式检查", command: "cargo", args: ["fmt", "--manifest-path", "server/Cargo.toml", "--all", "--", "--check"], detail: "检查 Rust 源码格式" },
      { group: "scripts", title: "构建与发布脚本测试", command: packageManager, args: ["run", "test:scripts"], detail: "收集 Node.js 测试" },
      { group: "scripts", title: "FPK 进程身份校验测试", command: "sh", args: ["scripts/verify/test-fnos-process-identity.sh"], detail: "运行进程身份 Shell 场景" },
      { group: "scripts", title: "FPK 启动孤儿进程对账测试", command: "sh", args: ["scripts/verify/test-fnos-startup-reconcile.sh"], detail: "运行启动残留进程安全对账场景" },
      { group: "scripts", title: "FPK 停止卸载收敛测试", command: "sh", args: ["scripts/verify/test-fnos-lifecycle-stop.sh"], detail: "运行信号升级与卸载失败关闭场景" },
      { group: "scripts", title: "FPK 服务就绪脚本测试", command: "sh", args: ["scripts/verify/test-fnos-readiness.sh"], detail: "运行服务就绪 Shell 场景" },
      { group: "rust", title: "Rust 测试（warnings as errors）", command: "node", args: ["scripts/verify/run-rust-tests.mjs"], detail: "收集 Rust 测试目标" },
      { group: "rust", title: "Rust 编译（warnings as errors）", command: "cargo", args: ["build", "--manifest-path", "server/Cargo.toml"], detail: "准备 Cargo 编译", activity: cargoProgressDetail },
      { group: "frontend", title: "前端单元测试", command: packageManager, args: ["run", "test:unit"], detail: "收集 Vitest 测试文件" },
      { group: "frontend", title: "前端类型检查与构建", command: packageManager, args: ["run", "build"], detail: "准备前端类型检查" },
    ];
if (!quick && reused.has("scripts")) {
  steps.push({ title: "日志维护文档检查", command: "node", args: ["--test", "scripts/tests/logging-docs.test.mjs"], detail: "核对轮转、脱敏和请求关联说明" });
}

const verificationStartedAt = performance.now();
for (const step of steps) {
  if (reused.has(step.group)) continue;
  const stepStartedAt = performance.now();
  await runStep(step);
  console.log(`<== ${step.title}通过（${formatDuration(performance.now() - stepStartedAt)}）`);
}

if (inputs) {
  if (JSON.stringify(inputs) !== JSON.stringify(verificationFingerprint())) throw new Error("验证期间源码或工具链发生变化，请重新执行验证。");
  if (reused.has("frontend") && !hasVerifiedBuild(inputs, "frontend")) throw new Error("验证期间 Web 产物发生变化，请重新执行验证。");
  saveVerification(inputs, pending);
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
