<script setup lang="ts">
import { NEmpty, NTag } from "naive-ui";
import { nextTick, ref, watch } from "vue";
import { useI18n } from "../../../i18n";
import type { DebugLogCategory, DebugLogEntry, DebugLogLevel } from "../types";

const props = defineProps<{
  logs: DebugLogEntry[];
  totalCount: number;
  active: boolean;
}>();

const { t } = useI18n();
const logListRef = ref<HTMLElement | null>(null);

watch(
  () => props.logs.length,
  () => {
    if (props.active) {
      void scrollToBottom();
    }
  },
);

async function scrollToBottom() {
  await nextTick();
  if (logListRef.value) {
    logListRef.value.scrollTop = logListRef.value.scrollHeight;
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
  const labels: Record<DebugLogLevel, string> = { info: "INFO", warn: "WARN", error: "ERROR" };
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

defineExpose({ scrollToBottom });
</script>

<template>
  <NEmpty v-if="totalCount === 0" :description="t('logs.empty')" />
  <NEmpty v-else-if="logs.length === 0" :description="t('logs.noFiltered')" />
  <div v-else ref="logListRef" class="log-list">
    <article v-for="log in logs" :key="log.id" class="log-entry" :class="`level-${log.level}`">
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
</template>

<style scoped src="./DebugLogList.css"></style>
