import { t, type TranslationKey } from "../../../i18n";
import type { DownloadTask, DownloadTaskStatus } from "../../../types/tasks";

export type TaskDisplayStatus = DownloadTaskStatus | "resolving" | "confirming";

export function formatTaskSize(size: number) {
  if (size <= 0) {
    return "0 B";
  }

  const units = ["B", "KB", "MB", "GB", "TB"];
  let value = size;
  let unitIndex = 0;

  while (value >= 1024 && unitIndex < units.length - 1) {
    value /= 1024;
    unitIndex += 1;
  }

  return `${value.toFixed(value >= 10 || unitIndex === 0 ? 0 : 1)} ${units[unitIndex]}`;
}

export function formatTaskSizePair(task: DownloadTask) {
  if (task.totalLength <= 0) {
    return `${formatTaskSize(task.completedLength)} / ${t("common.unknown")}`;
  }

  return `${formatTaskSize(task.completedLength)} / ${formatTaskSize(task.totalLength)}`;
}

export function formatTaskEta(task: DownloadTask) {
  if (task.downloadSpeed <= 0 || task.totalLength <= task.completedLength) {
    return "--";
  }

  const seconds = Math.ceil((task.totalLength - task.completedLength) / task.downloadSpeed);
  if (seconds < 60) {
    return `${seconds}s`;
  }

  const minutes = Math.floor(seconds / 60);
  const restSeconds = seconds % 60;
  return `${minutes}m ${restSeconds}s`;
}

export function formatTaskError(task: DownloadTask) {
  const code = task.errorCode ? t("task.errorCode", { code: task.errorCode }) : "";
  return `${code}${task.errorMessage || t("common.unknown")}`;
}

export function formatTaskProgress(task: DownloadTask) {
  if (task.totalLength <= 0) {
    return "0.00%";
  }

  const percentage = Math.min(100, (task.completedLength / task.totalLength) * 100);
  return `${percentage.toFixed(2)}%`;
}

export function deriveTaskDisplayStatus(task: DownloadTask): TaskDisplayStatus {
  if (task.confirmationRequired) {
    return "confirming";
  }
  if (task.url.toLowerCase().startsWith("magnet:?") && task.gid && task.files.length === 0) {
    return "resolving";
  }
  return task.status;
}

export function formatTaskStatusLabel(taskOrStatus: DownloadTask | DownloadTaskStatus) {
  const status =
    typeof taskOrStatus === "string" ? taskOrStatus : deriveTaskDisplayStatus(taskOrStatus);
  const labels: Record<TaskDisplayStatus, TranslationKey> = {
    pending: "task.status.pending",
    active: "task.status.active",
    paused: "task.status.paused",
    complete: "task.status.complete",
    error: "task.status.error",
    removed: "task.status.removed",
    resolving: "task.status.resolving",
    confirming: "task.status.confirming",
  };
  return t(labels[status]);
}
