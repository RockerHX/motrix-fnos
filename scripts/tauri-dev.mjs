#!/usr/bin/env node
import { spawn } from "node:child_process";
import path from "node:path";
import process from "node:process";

const isWindows = process.platform === "win32";
const tauriBin = path.join(process.cwd(), "node_modules", ".bin", isWindows ? "tauri.cmd" : "tauri");
const child = spawn(tauriBin, ["dev"], {
  cwd: process.cwd(),
  detached: !isWindows,
  stdio: ["ignore", "inherit", "inherit"],
  shell: isWindows,
});

let stopping = false;
let requestedQuit = false;
let forceTimer;

function stopChild(signal = "SIGINT", options = {}) {
  if (options.requestedQuit) {
    requestedQuit = true;
  }

  if (stopping) {
    return;
  }

  stopping = true;
  process.stdout.write("\n正在退出 Tauri dev...\n");

  try {
    if (isWindows) {
      child.kill(signal);
    } else {
      process.kill(-child.pid, signal);
    }
  } catch {
    child.kill(signal);
  }

  forceTimer = setTimeout(() => {
    try {
      if (isWindows) {
        child.kill("SIGTERM");
      } else {
        process.kill(-child.pid, "SIGTERM");
      }
    } catch {
      // 子进程已退出，无需处理。
    }
  }, 5000);
}

function restoreStdin() {
  if (!process.stdin.isTTY) {
    return;
  }

  process.stdin.setRawMode(false);
  process.stdin.pause();
}

if (process.stdin.isTTY) {
  process.stdout.write("按 q 退出 Tauri dev，或按 Ctrl+C 强制中断。\n");
  process.stdin.setRawMode(true);
  process.stdin.resume();
  process.stdin.setEncoding("utf8");
  process.stdin.on("data", (key) => {
    if (key === "q" || key === "Q") {
      stopChild("SIGINT", { requestedQuit: true });
      return;
    }

    if (key === "\u0003") {
      stopChild("SIGINT");
    }
  });
}

process.on("SIGINT", () => stopChild("SIGINT"));
process.on("SIGTERM", () => stopChild("SIGTERM"));

child.on("exit", (code, signal) => {
  if (forceTimer) {
    clearTimeout(forceTimer);
  }
  restoreStdin();

  if (requestedQuit) {
    process.exit(0);
  }

  if (signal) {
    process.exit(signal === "SIGINT" ? 130 : 143);
  }

  process.exit(code ?? 0);
});

child.on("error", (error) => {
  restoreStdin();
  console.error(`启动 Tauri dev 失败：${error.message}`);
  process.exit(1);
});
