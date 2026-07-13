<script setup lang="ts">
import { storeToRefs } from "pinia";
import { NButton, NCard, NEmpty, NInput, NModal, NSelect, NSwitch, NTag, useMessage } from "naive-ui";
import { computed, nextTick, ref, watch } from "vue";
import AppMetricGrid from "../../../components/ui/AppMetricGrid.vue";
import { useDebugLogStore } from "../stores/debugLogStore";
import { useI18n } from "../../../i18n";
import { getErrorMessage } from "../../../app/utils/errors";
import type { DebugLogCategory, DebugLogEntry, DebugLogLevel } from "../types";
import { useDebugLogFilters } from "../composables/useDebugLogFilters";
import { useDebugLogExport } from "../composables/useDebugLogExport";

const props = defineProps<{
  show: boolean;
}>();

const emit = defineEmits<{
  "update:show": [show: boolean];
}>();

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
const {
  levelFilter,
  categoryFilter,
  moduleFilter,
  searchText,
  onlyProblems,
  categoryOptions,
  levelOptions,
  moduleOptions,
  filteredLogs,
  logStats,
  logSummaryItems,
  clearFilters,
} = useDebugLogFilters(logs);
const { copyAllLogs, downloadAllLogs } = useDebugLogExport({
  logs,
  filteredLogs,
  warningCount: computed(() => logStats.value.warnings),
  errorCount: computed(() => logStats.value.errors),
  onManualCopy: showManualCopyDialog,
});

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

async function scrollToBottom() {
  await nextTick();
  const logList = logListRef.value;
  if (logList) {
    logList.scrollTop = logList.scrollHeight;
  }
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

      <AppMetricGrid class="log-metrics" :items="logSummaryItems" :desktop-columns="5" :mobile-columns="1" />

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

.log-metrics {
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
