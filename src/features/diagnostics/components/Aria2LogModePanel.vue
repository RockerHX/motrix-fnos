<script setup lang="ts">
import { NAlert, NButton, NSpace, NTag, useMessage } from "naive-ui";
import { computed, onBeforeUnmount, ref, watch } from "vue";
import AppIcon from "../../../components/AppIcon.vue";
import { getErrorMessage } from "../../../app/utils/errors";
import { useI18n } from "../../../i18n";
import { getAria2LogMode, updateAria2LogMode } from "../services/aria2LogModeService";
import type { Aria2LogModeStatus } from "../types";

const props = defineProps<{
  active: boolean;
}>();

const emit = defineEmits<{
  updated: [status: Aria2LogModeStatus];
}>();

const { t } = useI18n();
const message = useMessage();
const status = ref<Aria2LogModeStatus | null>(null);
const isLoading = ref(false);
const isUpdating = ref(false);
const errorMessage = ref("");
const nowMs = ref(Date.now());
let clockTimer: ReturnType<typeof setInterval> | null = null;

const detailedRemainingMs = computed(() => {
  const deadline = status.value?.detailedUntilMs;
  if (!status.value?.detailed || deadline === null || deadline === undefined) {
    return null;
  }
  return Math.max(0, deadline - nowMs.value);
});

const remainingLabel = computed(() => {
  const remaining = detailedRemainingMs.value;
  if (remaining === null) {
    return "";
  }
  const totalSeconds = Math.ceil(remaining / 1000);
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = String(totalSeconds % 60).padStart(2, "0");
  return t("diagnostics.logMode.remaining", { time: `${minutes}:${seconds}` });
});

const modeLabel = computed(() =>
  status.value?.detailed ? t("diagnostics.logMode.debug") : t("diagnostics.logMode.warn"),
);

const fileSizeLimitLabel = computed(() => {
  const bytes = status.value?.maxFileSizeBytes;
  if (bytes === undefined) {
    return "";
  }
  return formatMebibytes(bytes);
});

watch(
  () => props.active,
  (active) => {
    if (active) {
      void refresh();
    } else {
      stopClock();
    }
  },
  { immediate: true },
);

watch(
  () => status.value?.detailed,
  (detailed) => {
    if (detailed) {
      startClock();
    } else {
      stopClock();
    }
  },
);

onBeforeUnmount(stopClock);

async function refresh() {
  isLoading.value = true;
  errorMessage.value = "";
  try {
    status.value = await getAria2LogMode();
    nowMs.value = Date.now();
  } catch (error) {
    errorMessage.value = getErrorMessage(error, t("diagnostics.logMode.loadFailed"));
  } finally {
    isLoading.value = false;
  }
}

async function setDetailed(detailed: boolean) {
  isUpdating.value = true;
  errorMessage.value = "";
  try {
    const nextStatus = await updateAria2LogMode(detailed);
    status.value = nextStatus;
    nowMs.value = Date.now();
    emit("updated", nextStatus);
    message.success(
      detailed ? t("diagnostics.logMode.enabled") : t("diagnostics.logMode.restored"),
    );
  } catch (error) {
    errorMessage.value = getErrorMessage(error, t("diagnostics.logMode.updateFailed"));
  } finally {
    isUpdating.value = false;
  }
}

function startClock() {
  if (clockTimer !== null) {
    return;
  }
  clockTimer = setInterval(() => {
    nowMs.value = Date.now();
    if (detailedRemainingMs.value === 0 && !isLoading.value && !isUpdating.value) {
      void refresh();
    }
  }, 1000);
}

function stopClock() {
  if (clockTimer !== null) {
    clearInterval(clockTimer);
    clockTimer = null;
  }
}

function formatMebibytes(bytes: number) {
  const mebibytes = Math.max(0, bytes) / (1024 * 1024);
  return `${Number.isInteger(mebibytes) ? mebibytes : mebibytes.toFixed(1)} MiB`;
}
</script>

<template>
  <section class="aria2-log-mode-panel" aria-live="polite">
    <div class="aria2-log-mode-header">
      <div>
        <p class="aria2-log-mode-eyebrow">{{ t("diagnostics.logMode.eyebrow") }}</p>
        <h3>{{ t("diagnostics.logMode.title") }}</h3>
      </div>
      <NTag v-if="status" :type="status.detailed ? 'warning' : 'success'" size="small">
        {{ modeLabel }}
      </NTag>
    </div>

    <p class="aria2-log-mode-description">{{ t("diagnostics.logMode.description") }}</p>

    <div v-if="status" class="aria2-log-mode-meta">
      <span>{{ t("diagnostics.logMode.level", { level: modeLabel }) }}</span>
      <span v-if="status.detailed">{{ remainingLabel }}</span>
      <span>{{ t("diagnostics.logMode.limit", { size: fileSizeLimitLabel, count: status.maxFileCount }) }}</span>
    </div>

    <NAlert v-if="status?.appliesOnNextStart" type="info" :show-icon="false">
      {{ t("diagnostics.logMode.nextStart") }}
    </NAlert>

    <p v-if="errorMessage" class="aria2-log-mode-error">{{ errorMessage }}</p>

    <NSpace class="aria2-log-mode-actions" :wrap="true">
      <NButton
        type="primary"
        :loading="isUpdating || isLoading"
        :disabled="isUpdating || isLoading || status?.detailed === true"
        @click="setDetailed(true)"
      >
        <template #icon><AppIcon name="diagnostics" :size="16" /></template>
        {{ t("diagnostics.logMode.enable") }}
      </NButton>
      <NButton
        secondary
        :loading="isUpdating || isLoading"
        :disabled="isUpdating || isLoading || status?.detailed !== true"
        @click="setDetailed(false)"
      >
        <template #icon><AppIcon name="restore" :size="16" /></template>
        {{ t("diagnostics.logMode.restore") }}
      </NButton>
    </NSpace>
  </section>
</template>

<style scoped src="./Aria2LogModePanel.css"></style>
