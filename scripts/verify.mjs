#!/usr/bin/env node
import { spawn } from "node:child_process";
import process from "node:process";

const quick = process.argv.includes("--quick");
const packageManager = resolvePackageManager();
const rustEnv = {
  ...process.env,
  RUSTFLAGS: appendRustDenyWarnings(process.env.RUSTFLAGS),
};

const steps = quick
  ? [
      { title: "Rust 测试（warnings as errors）", command: "cargo", args: ["test", "--manifest-path", "src-tauri/Cargo.toml"], env: rustEnv },
      { title: "前端类型检查", command: packageManager, args: ["run", "typecheck"] },
    ]
  : [
      { title: "Rust 测试（warnings as errors）", command: "cargo", args: ["test", "--manifest-path", "src-tauri/Cargo.toml"], env: rustEnv },
      { title: "Rust 编译（warnings as errors）", command: "cargo", args: ["build", "--manifest-path", "src-tauri/Cargo.toml"], env: rustEnv },
      { title: "前端类型检查", command: packageManager, args: ["run", "typecheck"] },
      { title: "前端构建", command: packageManager, args: ["run", "build"] },
    ];

for (const step of steps) {
  await runStep(step);
}

console.log(quick ? "快速验证通过。" : "完整验证通过。");

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
