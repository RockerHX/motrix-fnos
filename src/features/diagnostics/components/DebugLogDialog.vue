<script setup lang="ts">
import { storeToRefs } from "pinia";
import { NButton, NCard, NEmpty, NInput, NModal, NSelect, NSwitch, NTag, useMessage } from "naive-ui";
import { computed, nextTick, ref, watch } from "vue";
import AppMetricGrid from "../../../components/ui/AppMetricGrid.vue";
import type { AppMetricItem } from "../../../components/ui/AppMetricGrid.vue";
import { useDebugLogStore } from "../stores/debugLogStore";
import { useI18n } from "../../../i18n";
import { getErrorMessage } from "../../../app/utils/errors";
import type { DebugLogCategory, DebugLogEntry, DebugLogLevel } from "../types";

const props = defineProps<{
  show: boolean;
}>();

const emit = defineEmits<{
  "update:show": [show: boolean];
}>();

type SelectOption = {
  label: string;
  value: string;
};

type ManualCopyInputRef = {
  focus?: () => void;
  select?: () => void;
  textareaElRef?: HTMLTextAreaElement | null;
  inputElRef?: HTMLInputElement | null;
  $el?: HTMLElement;
};

const propsShow = computed(() => props.show);
const message = useMessage();
const { t } = useI18n();
const debugLogStore = useDebugLogStore();
const { logs, isLoading, isClearing } = storeToRefs(debugLogStore);
const logListRef = ref<HTMLElement | null>(null);
const manualCopyRef = ref<ManualCopyInputRef | null>(null);
const showManualCopy = ref(false);
const manualCopyText = ref("");
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
    .sort((a, b) => a.localeCompare(b))
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
    moduleCounts.set(log.module, (moduleCounts.get(log.module) ?? 0) + repeatCount(log));
  }
  const topModule = [...moduleCounts.entries()].sort((a, b) => b[1] - a[1])[0]?.[0] ?? "-";
  return {
    total: logs.value.length,
    filtered: filteredLogs.value.length,
    errors,
    warnings,
    topModule,
  };
});
const logSummaryItems = computed<AppMetricItem[]>(() => [
  { label: t("logs.stats.total"), value: logStats.value.total },
  { label: t("logs.stats.filtered"), value: logStats.value.filtered },
  { label: t("logs.stats.warnings"), value: logStats.value.warnings, tone: logStats.value.warnings > 0 ? "warning" : "default" },
  { label: t("logs.stats.errors"), value: logStats.value.errors, tone: logStats.value.errors > 0 ? "error" : "default" },
  { label: t("logs.stats.topModule"), value: logStats.value.topModule },
]);

watch(
  propsShow,
  (show) => {
    if (show) {
      void refreshLogs();
    }
  },
);

watch(
  () => filteredLogs.value.length,
  () => {
    if (props.show) {
      void scrollToBottom();
    }
  },
);

function updateShow(show: boolean) {
  emit("update:show", show);
}

function closeDialog() {
  updateShow(false);
}

async function refreshLogs() {
  try {
    await debugLogStore.refreshLogs();
    await scrollToBottom();
  } catch (error) {
    message.error(getErrorMessage(error, t("common.unknown")));
  }
}

async function clearLogs() {
  try {
    await debugLogStore.clearLogs();
    clearFilters();
    message.success(t("logs.cleared"));
  } catch (error) {
    message.error(getErrorMessage(error, t("common.unknown")));
  }
}

function clearFilters() {
  levelFilter.value = null;
  categoryFilter.value = null;
  moduleFilter.value = null;
  searchText.value = "";
  onlyProblems.value = false;
}

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
    showManualCopyDialog(text);
    message.warning(t("logs.autoCopyLimited", { message: getErrorMessage(error, t("common.unknown")) }));
  }
}

async function copyText(text: string) {
  if (navigator.clipboard?.writeText) {
    await navigator.clipboard.writeText(text);
    return;
  }

  throw new Error(t("logs.clipboardUnavailable"));
}

function showManualCopyDialog(text: string) {
  manualCopyText.value = text;
  showManualCopy.value = true;
  void nextTick(() => {
    focusManualCopyInput();
  });
}

function closeManualCopyDialog() {
  showManualCopy.value = false;
}

function focusManualCopyInput() {
  const input = manualCopyRef.value;
  input?.focus?.();
  input?.select?.();

  const nativeInput =
    input?.textareaElRef ??
    input?.inputElRef ??
    (input?.$el?.querySelector?.("textarea, input") as HTMLTextAreaElement | HTMLInputElement | null | undefined);
  nativeInput?.focus();
  nativeInput?.select();
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
    `Motrix fnOS debug logs`,
    `Exported: ${new Date().toLocaleString()}`,
    `Total: ${logs.value.length}; Filtered: ${filteredLogs.value.length}; Warnings: ${logStats.value.warnings}; Errors: ${logStats.value.errors}`,
    "",
  ].join("\n");
  return `${header}${filteredLogs.value.map(formatLogLine).join("\n")}`;
}

async function scrollToBottom() {
  await nextTick();
  const logList = logListRef.value;
  if (logList) {
    logList.scrollTop = logList.scrollHeight;
  }
}

function formatLogLine(log: DebugLogEntry) {
  const repeats = repeatCount(log) > 1 ? ` x${repeatCount(log)} last=${formatTime(lastTimestampMs(log))}` : "";
  return `[${formatTime(log.timestampMs)}] [${log.level.toUpperCase()}] [${categoryLabel(log.category)}] [${log.module}]${repeats} ${log.message}`;
}

function formatTime(timestampMs: number) {
  return new Date(timestampMs).toLocaleString();
}

function repeatCount(log: DebugLogEntry) {
  return log.repeatCount ?? 1;
}

function lastTimestampMs(log: DebugLogEntry) {
  return log.lastTimestampMs ?? log.timestampMs;
}

function levelLabel(level: DebugLogLevel) {
  const labels: Record<DebugLogLevel, string> = {
    info: "INFO",
    warn: "WARN",
    error: "ERROR",
  };
  return labels[level];
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

function levelType(level: DebugLogLevel) {
  const types: Record<DebugLogLevel, "info" | "warning" | "error"> = {
    info: "info",
    warn: "warning",
    error: "error",
  };
  return types[level];
}
</script>

<template>
  <NModal :show="show" @update:show="updateShow">
    <NCard class="debug-log-dialog app-dialog" role="dialog" aria-modal="true">
      <template #header>
        <div>
          <p class="app-dialog-eyebrow">{{ t("logs.eyebrow") }}</p>
          <h2>{{ t("logs.title") }}</h2>
        </div>
      </template>
      <template #header-extra>
        <div class="debug-log-actions app-dialog-header-actions">
          <NButton size="small" secondary :loading="isLoading" @click="refreshLogs">{{ t("logs.refresh") }}</NButton>
          <NButton size="small" secondary @click="copyAllLogs">{{ t("logs.copyAll") }}</NButton>
          <NButton size="small" secondary @click="downloadAllLogs">{{ t("logs.download") }}</NButton>
          <NButton size="small" secondary @click="clearFilters">{{ t("logs.clearFilters") }}</NButton>
          <NButton size="small" secondary type="warning" :loading="isClearing" @click="clearLogs">{{ t("logs.clear") }}</NButton>
          <NButton quaternary circle :title="t('common.close')" :aria-label="t('common.close')" @click="closeDialog">×</NButton>
        </div>
      </template>

      <AppMetricGrid class="log-summary" :items="logSummaryItems" :desktop-columns="5" :mobile-columns="1" />

      <div class="log-filters">
        <NInput v-model:value="searchText" clearable :placeholder="t('logs.searchPlaceholder')" />
        <NSelect v-model:value="levelFilter" clearable :options="levelOptions" :placeholder="t('logs.levelFilter')" />
        <NSelect v-model:value="categoryFilter" clearable :options="categoryOptions" :placeholder="t('logs.categoryFilter')" />
        <NSelect v-model:value="moduleFilter" clearable filterable :options="moduleOptions" :placeholder="t('logs.moduleFilter')" />
        <label class="problem-toggle">
          <NSwitch v-model:value="onlyProblems" size="small" />
          <span>{{ t("logs.onlyProblems") }}</span>
        </label>
      </div>

      <NEmpty v-if="logs.length === 0" :description="t('logs.empty')" />
      <NEmpty v-else-if="filteredLogs.length === 0" :description="t('logs.noFiltered')" />
      <div v-else ref="logListRef" class="log-list">
        <article v-for="log in filteredLogs" :key="log.id" class="log-entry" :class="`level-${log.level}`">
          <div class="log-meta">
            <span>{{ formatTime(log.timestampMs) }}</span>
            <span v-if="repeatCount(log) > 1">{{ t("logs.repeated", { count: repeatCount(log) }) }}</span>
            <span v-if="repeatCount(log) > 1">{{ t("logs.lastSeen", { time: formatTime(lastTimestampMs(log)) }) }}</span>
            <NTag :type="levelType(log.level)" size="small" round>{{ levelLabel(log.level) }}</NTag>
            <NTag size="small" round>{{ categoryLabel(log.category) }}</NTag>
            <code>{{ log.module }}</code>
          </div>
          <p>{{ log.message }}</p>
        </article>
      </div>
    </NCard>
  </NModal>

  <NModal :show="showManualCopy" @update:show="showManualCopy = $event">
    <NCard class="manual-copy-dialog app-dialog" role="dialog" aria-modal="true">
      <template #header>
        <div>
          <p class="app-dialog-eyebrow">{{ t("logs.manualCopy.eyebrow") }}</p>
          <h2>{{ t("logs.manualCopy.title") }}</h2>
        </div>
      </template>
      <template #header-extra>
        <NButton quaternary circle :title="t('common.close')" :aria-label="t('common.close')" @click="closeManualCopyDialog">×</NButton>
      </template>

      <p class="manual-copy-hint">{{ t("logs.manualCopy.hint") }}</p>
      <NInput
        ref="manualCopyRef"
        class="manual-copy-input"
        type="textarea"
        readonly
        :value="manualCopyText"
        :input-props="{ readonly: true }"
        :autosize="{ minRows: 12, maxRows: 24 }"
      />
      <div class="manual-copy-actions">
        <NButton secondary @click="downloadAllLogs">{{ t("logs.download") }}</NButton>
        <NButton type="primary" @click="closeManualCopyDialog">{{ t("common.done") }}</NButton>
      </div>
    </NCard>
  </NModal>
</template>

<style scoped>
.debug-log-dialog {
  --app-dialog-width: 1120px;
}

.manual-copy-dialog {
  --app-dialog-width: 900px;
}

h2 {
  margin: 0;
}

.debug-log-actions :deep(.n-button) {
  white-space: normal;
}

.log-summary {
  margin-bottom: 12px;
}

.log-filters {
  display: grid;
  grid-template-columns: minmax(180px, 1.5fr) minmax(120px, 0.8fr) minmax(140px, 0.9fr) minmax(160px, 1fr) auto;
  gap: 10px;
  align-items: center;
  margin-bottom: 12px;
}

.problem-toggle {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  color: var(--app-text-muted);
  white-space: nowrap;
}

.log-list {
  max-height: min(620px, calc(100vh - 310px));
  overflow: auto;
  display: grid;
  gap: 10px;
  padding-right: 6px;
}

.log-entry {
  padding: 12px;
  border: 1px solid var(--app-color-border-subtle);
  border-left: 3px solid #5d7280;
  border-radius: var(--app-radius-sm);
  background: var(--app-color-card-overlay-subtle);
}

.log-entry.level-warn {
  border-left-color: #f2c97d;
}

.log-entry.level-error {
  border-left-color: var(--app-text-danger);
}

.log-meta {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 8px;
  color: #8d9c96;
  font-size: 12px;
}

.log-meta code {
  color: #9dd7ff;
}

.log-entry p {
  margin: 8px 0 0;
  color: #edf5ef;
  line-height: 1.6;
  white-space: pre-wrap;
  word-break: break-word;
}

.manual-copy-hint {
  margin: 0 0 12px;
  color: #b8c4be;
  line-height: 1.6;
}

.manual-copy-input {
  width: 100%;
}

.manual-copy-input :deep(textarea),
.manual-copy-input :deep(.n-input__textarea-el) {
  min-height: min(460px, calc(100vh - 260px));
  font: 12px/1.6 ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, "Liberation Mono", monospace;
}

.manual-copy-actions {
  display: flex;
  justify-content: flex-end;
  gap: 10px;
  margin-top: 14px;
}

@media (max-width: 900px) {
  .log-filters {
    grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
  }

  .problem-toggle {
    justify-content: flex-start;
  }
}

@media (max-width: 767px) {
  .debug-log-actions :deep(.n-button) {
    min-width: 0;
  }

  .log-filters {
    grid-template-columns: minmax(0, 1fr);
  }

  .log-list {
    max-height: calc(var(--app-viewport-height) - 430px);
    padding-right: 0;
  }

  .log-meta {
    gap: 6px;
  }

  .manual-copy-input :deep(textarea),
  .manual-copy-input :deep(.n-input__textarea-el) {
    min-height: calc(var(--app-viewport-height) - 360px);
    font-size: 16px;
  }

  .manual-copy-actions {
    flex-direction: column-reverse;
  }

  .manual-copy-actions :deep(.n-button) {
    width: 100%;
  }
}
</style>
