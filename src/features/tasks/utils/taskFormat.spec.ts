import { describe, expect, it } from "vitest";
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
    expect(formatTaskEta(createTask({ downloadSpeed: 10, totalLength: 100, completedLength: 70 }))).toBe("3s");
    expect(formatTaskEta(createTask({ downloadSpeed: 10, totalLength: 1000, completedLength: 0 }))).toBe("1m 40s");
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
    confirmationRequired: false,
    files: [],
    createdAt: 1,
    updatedAt: 1,
    ...overrides,
  };
}
