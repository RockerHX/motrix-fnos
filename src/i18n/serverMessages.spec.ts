import { afterEach, describe, expect, it } from "vitest";
import { setLanguage } from ".";
import { enUS } from "./locales/en-US";
import { zhCN } from "./locales/zh-CN";
import { formatServerMessage } from "./serverMessages";

describe("server messages", () => {
  afterEach(() => setLanguage("zh-CN"));

  it("keeps translation keys and parameters consistent between languages", () => {
    expect(Object.keys(enUS).sort()).toEqual(Object.keys(zhCN).sort());
    for (const key of Object.keys(zhCN) as (keyof typeof zhCN)[]) {
      const parameters = (value: string) => [...value.matchAll(/\{(\w+)\}/g)].map((match) => match[1]).sort();
      expect(parameters(enUS[key]), key).toEqual(parameters(zhCN[key]));
    }
  });

  it("preserves original Chinese messages and localizes known codes in English", () => {
    const original = "任务保存目录当前未获授权：/共享/电影";
    expect(formatServerMessage(original, "save_dir_not_authorized")).toBe(original);
    setLanguage("en-US");
    expect(formatServerMessage(original, "save_dir_not_authorized")).toContain("not currently authorized");
    expect(formatServerMessage("task_pause_unconfirmed：暂停超时", "task_operation_failed")).toContain("Stop and restart the app");
    expect(formatServerMessage("task_resume_state_invalid：原 GID 缺失")).toContain("Keep the files");
    expect(formatServerMessage("原始错误", "new_server_code")).toContain("new_server_code");
  });

  it("uses response state for normal messages and preserves external diagnostics", () => {
    setLanguage("en-US");
    expect(formatServerMessage("原始状态", "rpc_connected", { version: "2.8.6" })).toBe("Aria2 RPC is connected, version 2.8.6");
    expect(formatServerMessage("原始状态", "engine_exited", { pid: 123 })).toContain("PID 123");
    expect(formatServerMessage("原始状态", "update_up_to_date")).toBe("You are using the latest version.");
    expect(formatServerMessage("Permission denied: /共享/电影.mkv")).toBe("Permission denied: /共享/电影.mkv");
    expect(formatServerMessage("HTTP 503", "future_code")).toBe("HTTP 503");
  });
});
