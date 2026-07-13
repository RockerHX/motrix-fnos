import { computed, ref, type Ref } from "vue";
import type { AppMetricItem } from "../../../components/ui/AppMetricGrid.vue";
import { useI18n } from "../../../i18n";
import type { DebugLogCategory, DebugLogEntry, DebugLogLevel } from "../types";

type SelectOption = {
  label: string;
  value: string;
};

export function useDebugLogFilters(logs: Ref<DebugLogEntry[]>) {
  const { t } = useI18n();
  const levelFilter = ref<DebugLogLevel | null>(null);
  const categoryFilter = ref<DebugLogCategory | null>(null);
  const moduleFilter = ref<string | null>(null);
  const searchText = ref("");
  const onlyProblems = ref(false);

  const categoryOptions = computed<SelectOption[]>(() => [
    { label: t("logs.category.app"), value: "app" },
    { label: t("logs.category.task"), value: "task" },
    { label: t("logs.category.aria2"), value: "aria2" },
    { label: t("logs.category.settings"), value: "settings" },
    { label: t("logs.category.storage"), value: "storage" },
    { label: t("logs.category.api"), value: "api" },
    { label: t("logs.category.runtime"), value: "runtime" },
  ]);
  const levelOptions = computed<SelectOption[]>(() => [
    { label: "INFO", value: "info" },
    { label: "WARN", value: "warn" },
    { label: "ERROR", value: "error" },
  ]);
  const moduleOptions = computed<SelectOption[]>(() =>
    [...new Set(logs.value.map((log) => log.module).filter(Boolean))]
      .sort((left, right) => left.localeCompare(right))
      .map((module) => ({ label: module, value: module })),
  );
  const filteredLogs = computed(() => {
    const keyword = searchText.value.trim().toLowerCase();
    return logs.value.filter((log) => {
      if (onlyProblems.value && log.level === "info") return false;
      if (levelFilter.value && log.level !== levelFilter.value) return false;
      if (categoryFilter.value && log.category !== categoryFilter.value) return false;
      if (moduleFilter.value && log.module !== moduleFilter.value) return false;
      if (!keyword) return true;
      return [log.module, log.category, log.message].some((value) => value.toLowerCase().includes(keyword));
    });
  });
  const logStats = computed(() => {
    const errors = logs.value.filter((log) => log.level === "error").length;
    const warnings = logs.value.filter((log) => log.level === "warn").length;
    const moduleCounts = new Map<string, number>();
    for (const log of logs.value) {
      moduleCounts.set(log.module, (moduleCounts.get(log.module) ?? 0) + (log.repeatCount ?? 1));
    }
    const topModule = [...moduleCounts.entries()].sort((left, right) => right[1] - left[1])[0]?.[0] ?? "-";
    return { total: logs.value.length, filtered: filteredLogs.value.length, errors, warnings, topModule };
  });
  const logSummaryItems = computed<AppMetricItem[]>(() => [
    { label: t("logs.stats.total"), value: logStats.value.total },
    { label: t("logs.stats.filtered"), value: logStats.value.filtered },
    { label: t("logs.stats.warnings"), value: logStats.value.warnings, tone: logStats.value.warnings > 0 ? "warning" : "default" },
    { label: t("logs.stats.errors"), value: logStats.value.errors, tone: logStats.value.errors > 0 ? "error" : "default" },
    { label: t("logs.stats.topModule"), value: logStats.value.topModule },
  ]);

  function clearFilters() {
    levelFilter.value = null;
    categoryFilter.value = null;
    moduleFilter.value = null;
    searchText.value = "";
    onlyProblems.value = false;
  }

  return {
    levelFilter, categoryFilter, moduleFilter, searchText, onlyProblems,
    categoryOptions, levelOptions, moduleOptions, filteredLogs, logStats, logSummaryItems, clearFilters,
  };
}

