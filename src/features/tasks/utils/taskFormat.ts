import { t, type TranslationKey } from "../../../i18n";
import type { DownloadTask, DownloadTaskSourceType, DownloadTaskStatus } from "../../../types/tasks";

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
  if (deriveTaskDisplayStatus(task) === "resolving") {
    return t("common.notAvailable");
  }
  if (task.totalLength <= 0) {
    return `${formatTaskSize(task.completedLength)} / ${t("common.unknown")}`;
  }

  return `${formatTaskSize(task.completedLength)} / ${formatTaskSize(task.totalLength)}`;
}

export function formatTaskSpeed(task: DownloadTask) {
  if (deriveTaskDisplayStatus(task) !== "active" || !Number.isFinite(task.downloadSpeed) || task.downloadSpeed < 0) {
    return t("common.notAvailable");
  }
  return `${formatTaskSize(task.downloadSpeed)}/s`;
}

export function formatTaskEta(task: DownloadTask) {
  if (deriveTaskDisplayStatus(task) !== "active" || task.downloadSpeed <= 0 || task.totalLength <= task.completedLength) {
    return t("common.notAvailable");
  }

  const seconds = Math.ceil((task.totalLength - task.completedLength) / task.downloadSpeed);
  if (!Number.isFinite(seconds) || seconds <= 0) {
    return t("common.notAvailable");
  }

  const parts: [number, TranslationKey][] = [
    [Math.floor(seconds / 86400), "task.eta.days"],
    [Math.floor(seconds / 3600) % 24, "task.eta.hours"],
    [Math.floor(seconds / 60) % 60, "task.eta.minutes"],
    [seconds % 60, "task.eta.seconds"],
  ];
  const firstUnit = parts.findIndex(([value]) => value > 0);
  const duration = parts.slice(firstUnit, firstUnit + 2)
    .filter(([value]) => value > 0)
    .map(([value, key]) => t(key, { value }))
    .join(" ");
  return t("task.eta.approximate", { duration });
}

export function formatTaskError(task: DownloadTask) {
  const code = task.errorCode ? t("task.errorCode", { code: task.errorCode }) : "";
  return `${code}${task.errorMessage || t("common.unknown")}`;
}

export function getTaskSourceType(task: Pick<DownloadTask, "sourceType" | "url">): DownloadTaskSourceType {
  if (task.sourceType) return task.sourceType;
  const url = task.url.trim().toLowerCase();
  if (url.startsWith("magnet:?")) return "magnet";
  if (url.startsWith("torrent:")) return "torrent";
  return "url";
}

export function canDeleteTaskFiles(task: DownloadTask): boolean {
  return !task.filesDeleted && (getTaskSourceType(task) !== "magnet"
    || Boolean(task.ownedTaskDir?.trim())
    || (!task.confirmationRequired && Boolean(task.filePath?.trim())));
}

export function getTaskProgressPercentage(task: DownloadTask) {
  const status = deriveTaskDisplayStatus(task);
  if (
    !Number.isFinite(task.totalLength) || task.totalLength <= 0 ||
    !Number.isFinite(task.completedLength) || status === "resolving" || status === "confirming"
  ) {
    return null;
  }

  const maximum = task.status === "complete" || (task.status === "removed" && task.completedLength >= task.totalLength)
    ? 100 : 99.9;
  return Math.max(0, Math.min(maximum, (task.completedLength / task.totalLength) * 100));
}

export function formatTaskProgress(task: DownloadTask) {
  const percentage = getTaskProgressPercentage(task);
  return percentage === null ? t("common.notAvailable") : `${percentage.toFixed(1)}%`;
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
  if (typeof taskOrStatus !== "string" && taskOrStatus.cleanupState && taskOrStatus.cleanupState !== "none") {
    return t(taskOrStatus.cleanupState === "failed" ? "task.status.cleanupFailed" : "task.status.cleanupPending");
  }
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
