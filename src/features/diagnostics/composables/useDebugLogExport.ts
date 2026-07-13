import { useMessage } from "naive-ui";
import type { Ref } from "vue";
import { getErrorMessage } from "../../../app/utils/errors";
import { useI18n } from "../../../i18n";
import type { DebugLogCategory, DebugLogEntry } from "../types";

interface UseDebugLogExportOptions {
  logs: Ref<DebugLogEntry[]>;
  filteredLogs: Ref<DebugLogEntry[]>;
  warningCount: Ref<number>;
  errorCount: Ref<number>;
  onManualCopy: (text: string) => void;
}

export function useDebugLogExport({
  logs,
  filteredLogs,
  warningCount,
  errorCount,
  onManualCopy,
}: UseDebugLogExportOptions) {
  const message = useMessage();
  const { t } = useI18n();

  async function copyAllLogs() {
    if (filteredLogs.value.length === 0) {
      message.warning(t("logs.noCopy"));
      return;
    }

    const text = formatAllLogs();
    try {
      await copyText(text);
      message.success(t("logs.copied"));
    } catch (error) {
      onManualCopy(text);
      message.warning(t("logs.autoCopyLimited", { message: getErrorMessage(error, t("common.unknown")) }));
    }
  }

  function downloadAllLogs() {
    if (filteredLogs.value.length === 0) {
      message.warning(t("logs.noDownload"));
      return;
    }

    const blob = new Blob([formatAllLogs()], { type: "text/plain;charset=utf-8" });
    const url = URL.createObjectURL(blob);
    const link = document.createElement("a");
    link.href = url;
    link.download = `motrix-fnos-debug-${new Date().toISOString().replace(/[:.]/g, "-")}.log`;
    document.body.appendChild(link);
    link.click();
    document.body.removeChild(link);
    URL.revokeObjectURL(url);
    message.success(t("logs.exported"));
  }

  function formatAllLogs() {
    const header = [
      "Motrix fnOS debug logs",
      `Exported: ${new Date().toLocaleString()}`,
      `Total: ${logs.value.length}; Filtered: ${filteredLogs.value.length}; Warnings: ${warningCount.value}; Errors: ${errorCount.value}`,
      "",
    ].join("\n");
    return `${header}${filteredLogs.value.map(formatLogLine).join("\n")}`;
  }

  async function copyText(text: string) {
    if (navigator.clipboard?.writeText) {
      await navigator.clipboard.writeText(text);
      return;
    }

    throw new Error(t("logs.clipboardUnavailable"));
  }

  function formatLogLine(log: DebugLogEntry) {
    const repeatCount = log.repeatCount ?? 1;
    const repeats = repeatCount > 1 ? ` x${repeatCount} last=${formatTime(log.lastTimestampMs ?? log.timestampMs)}` : "";
    return `[${formatTime(log.timestampMs)}] [${log.level.toUpperCase()}] [${categoryLabel(log.category)}] [${log.module}]${repeats} ${log.message}`;
  }

  function categoryLabel(category: DebugLogCategory) {
    const labels: Record<DebugLogCategory, string> = {
      app: t("logs.category.app"),
      task: t("logs.category.task"),
      aria2: t("logs.category.aria2"),
      settings: t("logs.category.settings"),
      storage: t("logs.category.storage"),
      api: t("logs.category.api"),
      runtime: t("logs.category.runtime"),
    };
    return labels[category] ?? category;
  }

  function formatTime(timestampMs: number) {
    return new Date(timestampMs).toLocaleString();
  }

  return { copyAllLogs, downloadAllLogs, formatAllLogs };
}
