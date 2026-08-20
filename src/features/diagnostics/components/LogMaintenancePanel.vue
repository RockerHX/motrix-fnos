<script setup lang="ts">
import { NAlert, NButton, NPopconfirm, NSpace, useMessage } from "naive-ui";
import { computed, ref, watch } from "vue";
import AppIcon from "../../../components/AppIcon.vue";
import AppMetricGrid from "../../../components/ui/AppMetricGrid.vue";
import type { AppMetricItem } from "../../../components/ui/appMetric";
import { getErrorMessage } from "../../../app/utils/errors";
import { useI18n } from "../../../i18n";
import { clearAria2Logs, getLogUsage } from "../services/logMaintenanceService";
import type { DiagnosticsLogUsage, LogFileUsage } from "../types";

const LOG_USAGE_WARNING_BYTES = 80 * 1024 * 1024;

const props = defineProps<{
  active: boolean;
  aria2Running: boolean | null;
}>();

const emit = defineEmits<{
  updated: [usage: DiagnosticsLogUsage];
}>();

const { t } = useI18n();
const message = useMessage();
const usage = ref<DiagnosticsLogUsage | null>(null);
const isLoading = ref(false);
const isClearing = ref(false);
const errorMessage = ref("");

const canClear = computed(
  () => props.aria2Running === false && !isLoading.value && !isClearing.value,
);
const isUsageWarning = computed(
  () => usage.value !== null && usage.value.totalBytes >= LOG_USAGE_WARNING_BYTES,
);
const usageItems = computed<AppMetricItem[]>(() => {
  const snapshot = usage.value;
  if (!snapshot) {
    return [];
  }

  return [
    {
      label: t("diagnostics.logUsage.total"),
      value: formatBytes(snapshot.totalBytes),
      detail: t("diagnostics.logUsage.files", { count: snapshot.totalFileCount }),
      tone: isUsageWarning.value ? "warning" : "default",
    },
    usageMetric("diagnostics.logUsage.aria2", snapshot.aria2),
    usageMetric("diagnostics.logUsage.server", snapshot.server),
    usageMetric("diagnostics.logUsage.lifecycle", snapshot.lifecycle),
  ];
});

watch(
  () => props.active,
  (active) => {
    if (active) {
      void refresh();
    }
  },
  { immediate: true },
);

async function refresh() {
  isLoading.value = true;
  errorMessage.value = "";
  try {
    usage.value = await getLogUsage();
  } catch (error) {
    errorMessage.value = getErrorMessage(error, t("diagnostics.logUsage.loadFailed"));
  } finally {
    isLoading.value = false;
  }
}

async function clearLogs() {
  if (!canClear.value) {
    return false;
  }

  isClearing.value = true;
  errorMessage.value = "";
  try {
    const response = await clearAria2Logs();
    usage.value = response.usage;
    emit("updated", response.usage);
    message.success(t("diagnostics.logUsage.clearSuccess", { size: formatBytes(response.reclaimedBytes) }));
    return true;
  } catch (error) {
    errorMessage.value = getErrorMessage(error, t("diagnostics.logUsage.clearFailed"));
    return false;
  } finally {
    isClearing.value = false;
  }
}

function usageMetric(label: "diagnostics.logUsage.aria2" | "diagnostics.logUsage.server" | "diagnostics.logUsage.lifecycle", item: LogFileUsage): AppMetricItem {
  return {
    label: t(label),
    value: formatBytes(item.totalBytes),
    detail: t("diagnostics.logUsage.currentHistory", {
      current: formatBytes(item.currentBytes),
      history: formatBytes(item.historyBytes),
    }),
    note: t("diagnostics.logUsage.files", { count: item.totalFileCount }),
  };
}

function formatBytes(bytes: number) {
  const normalized = Math.max(0, bytes);
  if (normalized < 1024) {
    return `${normalized} B`;
  }

  const units = ["KiB", "MiB", "GiB"];
  let value = normalized / 1024;
  let unitIndex = 0;
  while (value >= 1024 && unitIndex < units.length - 1) {
    value /= 1024;
    unitIndex += 1;
  }
  return `${Number.isInteger(value) ? value : value.toFixed(1)} ${units[unitIndex]}`;
}

defineExpose({ refresh });
</script>

<template>
  <section class="log-maintenance-panel" aria-live="polite">
    <div class="log-maintenance-header">
      <div>
        <p class="log-maintenance-eyebrow">{{ t("diagnostics.logUsage.eyebrow") }}</p>
        <h3>{{ t("diagnostics.logUsage.title") }}</h3>
      </div>
      <NButton
        quaternary
        circle
        :loading="isLoading"
        :disabled="isClearing"
        :title="t('diagnostics.logUsage.refresh')"
        :aria-label="t('diagnostics.logUsage.refresh')"
        @click="refresh"
      >
        <template #icon><AppIcon name="refresh" :size="16" /></template>
      </NButton>
    </div>

    <p class="log-maintenance-description">{{ t("diagnostics.logUsage.description") }}</p>

    <NAlert v-if="isUsageWarning" type="warning" :show-icon="false">
      {{ t("diagnostics.logUsage.warning") }}
    </NAlert>
    <NAlert v-else-if="props.aria2Running === true" type="info" :show-icon="false">
      {{ t("diagnostics.logUsage.engineRunning") }}
    </NAlert>
    <NAlert v-else-if="props.aria2Running === null" type="info" :show-icon="false">
      {{ t("diagnostics.logUsage.engineUnknown") }}
    </NAlert>

    <AppMetricGrid v-if="usage" :items="usageItems" :desktop-columns="4" :mobile-columns="1" />
    <p v-else-if="isLoading" class="log-maintenance-loading">{{ t("common.loading") }}</p>
    <p v-if="errorMessage" class="log-maintenance-error">{{ errorMessage }}</p>

    <NSpace class="log-maintenance-actions" :wrap="true">
      <NPopconfirm
        :disabled="!canClear"
        :positive-text="t('diagnostics.logUsage.clearConfirmAction')"
        :negative-text="t('common.cancel')"
        :positive-button-props="{ type: 'error' }"
        @positive-click="clearLogs"
      >
        <template #trigger>
          <NButton type="error" secondary :loading="isClearing" :disabled="!canClear">
            <template #icon><AppIcon name="trash" :size="16" /></template>
            {{ t("diagnostics.logUsage.clear") }}
          </NButton>
        </template>
        {{ t("diagnostics.logUsage.clearConfirm") }}
      </NPopconfirm>
    </NSpace>
  </section>
</template>

<style scoped src="./LogMaintenancePanel.css"></style>
