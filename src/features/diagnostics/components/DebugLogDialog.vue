<script setup lang="ts">
import { storeToRefs } from "pinia";
import { NAlert, NButton, NCard, NInput, NModal, NSelect, NSwitch, useMessage } from "naive-ui";
import { computed, ref, watch } from "vue";
import AppMetricGrid from "../../../components/ui/AppMetricGrid.vue";
import { useDebugLogStore } from "../stores/debugLogStore";
import { useI18n } from "../../../i18n";
import { getErrorMessage } from "../../../app/utils/errors";
import { useDebugLogFilters } from "../composables/useDebugLogFilters";
import { useDebugLogExport } from "../composables/useDebugLogExport";
import DebugLogManualCopyDialog from "./DebugLogManualCopyDialog.vue";
import DebugLogList from "./DebugLogList.vue";

const props = defineProps<{
  show: boolean;
}>();

const emit = defineEmits<{
  "update:show": [show: boolean];
}>();

const propsShow = computed(() => props.show);
const message = useMessage();
const { t } = useI18n();
const debugLogStore = useDebugLogStore();
const { logs, isLoading, isClearing } = storeToRefs(debugLogStore);
const logListRef = ref<InstanceType<typeof DebugLogList> | null>(null);
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
}

async function scrollToBottom() {
  await logListRef.value?.scrollToBottom();
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

      <NAlert class="debug-log-storage-note" type="info" :show-icon="false">
        {{ t("logs.storageNote") }}
      </NAlert>

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

      <DebugLogList ref="logListRef" :logs="filteredLogs" :total-count="logs.length" :active="show" />
    </NCard>
  </NModal>

  <DebugLogManualCopyDialog
    v-model:show="showManualCopy"
    :text="manualCopyText"
    @download="downloadAllLogs"
  />
</template>

<style scoped src="./DebugLogDialog.css"></style>
