import { afterEach, describe, expect, it } from "vitest";
import { setLanguage } from "../../../i18n";
import type { DownloadTask, DownloadTaskStatus } from "../../../types/tasks";
import {
  formatTaskError,
  formatTaskEta,
  formatTaskProgress,
  formatTaskSize,
  formatTaskSizePair,
  formatTaskStatusLabel,
} from "./taskFormat";

describe("taskFormat", () => {
  afterEach(() => setLanguage("zh-CN"));

  it("formats byte sizes with existing unit rules", () => {
    expect(formatTaskSize(-1)).toBe("0 B");
    expect(formatTaskSize(0)).toBe("0 B");
    expect(formatTaskSize(512)).toBe("512 B");
    expect(formatTaskSize(1024)).toBe("1.0 KB");
    expect(formatTaskSize(10 * 1024)).toBe("10 KB");
    expect(formatTaskSize(1024 ** 2)).toBe("1.0 MB");
    expect(formatTaskSize(1024 ** 3)).toBe("1.0 GB");
    expect(formatTaskSize(1024 ** 4)).toBe("1.0 TB");
  });

  it("formats size pairs with unknown total length", () => {
    expect(formatTaskSizePair(createTask({ completedLength: 2048, totalLength: 0 }))).toBe("2.0 KB / 未知");
    expect(formatTaskSizePair(createTask({ completedLength: 2048, totalLength: 4096 }))).toBe("2.0 KB / 4.0 KB");
  });

  it("formats eta for unavailable, seconds and minutes cases", () => {
    expect(formatTaskEta(createTask({ downloadSpeed: 0, totalLength: 100, completedLength: 0 }))).toBe("--");
    expect(formatTaskEta(createTask({ downloadSpeed: 10, totalLength: 100, completedLength: 100 }))).toBe("--");
    expect(formatTaskEta(createTask({ downloadSpeed: 10, totalLength: 100, completedLength: 70 }))).toBe("约 3秒");
    expect(formatTaskEta(createTask({ downloadSpeed: 10, totalLength: 1000, completedLength: 0 }))).toBe("约 1分 40秒");
  });

  it.each([
    [0.1, "约 1秒"],
    [59, "约 59秒"],
    [60, "约 1分"],
    [61, "约 1分 1秒"],
    [3599, "约 59分 59秒"],
    [3600, "约 1小时"],
    [3661, "约 1小时 1分"],
    [86399, "约 23小时 59分"],
    [86400, "约 1天"],
    [90061, "约 1天 1小时"],
    [172800, "约 2天"],
  ])("formats %s seconds using at most two adjacent units", (seconds, expected) => {
    expect(formatTaskEta(createTask({ downloadSpeed: 1, totalLength: seconds }))).toBe(expected);
  });

  it.each([
    [59, "About 59s"],
    [61, "About 1m 1s"],
    [3661, "About 1h 1m"],
    [90061, "About 1d 1h"],
  ])("localizes %s seconds in English", (seconds, expected) => {
    setLanguage("en-US");
    expect(formatTaskEta(createTask({ downloadSpeed: 1, totalLength: seconds }))).toBe(expected);
  });

  it.each([NaN, Infinity])("does not estimate ETA from invalid speed %s", (downloadSpeed) => {
    expect(formatTaskEta(createTask({ downloadSpeed }))).toBe("--");
  });

  it("formats task errors with code and fallback", () => {
    expect(formatTaskError(createTask({ errorCode: "3", errorMessage: "disk full" }))).toBe("错误码 3：disk full");
    expect(formatTaskError(createTask({ errorCode: null, errorMessage: "network lost" }))).toBe("network lost");
    expect(formatTaskError(createTask({ errorCode: "5", errorMessage: "" }))).toBe("错误码 5：未知");
  });

  it("formats progress with unknown total and caps at 100 percent", () => {
    expect(formatTaskProgress(createTask({ totalLength: 0, completedLength: 10 }))).toBe("0.00%");
    expect(formatTaskProgress(createTask({ totalLength: 1000, completedLength: 123 }))).toBe("12.30%");
    expect(formatTaskProgress(createTask({ totalLength: 100, completedLength: 120 }))).toBe("100.00%");
  });

  it("shows pending and failed file cleanup on Trash tasks", () => {
    expect(formatTaskStatusLabel(createTask({ status: "removed", cleanupState: "pending" }))).toBe("文件清理中");
    expect(formatTaskStatusLabel(createTask({ status: "removed", cleanupState: "failed" }))).toBe("文件清理失败，将自动重试");
  });

  it("formats every task status label", () => {
    const labels: Record<DownloadTaskStatus, string> = {
      pending: "排队",
      active: "下载中",
      paused: "暂停",
      complete: "已完成",
      error: "错误",
      removed: "已删除",
    };

    for (const [status, label] of Object.entries(labels)) {
      expect(formatTaskStatusLabel(status as DownloadTaskStatus)).toBe(label);
    }
  });

  it("derives resolving and confirming labels for magnet tasks", () => {
    expect(
      formatTaskStatusLabel(
        createTask({
          url: "magnet:?xt=urn:btih:test",
          status: "pending",
          gid: "gid-1",
          files: [],
        }),
      ),
    ).toBe("解析中");
    expect(
      formatTaskStatusLabel(
        createTask({
          url: "magnet:?xt=urn:btih:test",
          status: "pending",
          gid: null,
          confirmationRequired: true,
          files: [{ index: 1, path: "/downloads/file.iso", name: "file.iso", length: 1, completedLength: 0, selected: true }],
        }),
      ),
    ).toBe("待确认");
  });
});

function createTask(overrides: Partial<DownloadTask> = {}): DownloadTask {
  return {
    id: 1,
    url: "https://example.com/file.iso",
    fileName: "file.iso",
    saveDir: "/downloads",
    category: "默认",
    gid: "gid-1",
    status: "active",
    totalLength: 100,
    completedLength: 0,
    downloadSpeed: 0,
    errorCode: null,
    errorMessage: null,
    filePath: null,
    useProxy: false,
    confirmationRequired: false,
    files: [],
    createdAt: 1,
    updatedAt: 1,
    ...overrides,
  };
}
