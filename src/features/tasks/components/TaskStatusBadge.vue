<script setup lang="ts">
import { computed } from "vue";
import { NTag } from "naive-ui";
import { useI18n, type TranslationKey } from "../../../i18n";
import type { DownloadTaskStatus } from "../../../types/tasks";

const props = defineProps<{
  status: DownloadTaskStatus;
}>();

const { t } = useI18n();

const label = computed(() => {
  const labels: Record<DownloadTaskStatus, TranslationKey> = {
    pending: "task.status.pending",
    active: "task.status.active",
    paused: "task.status.paused",
    complete: "task.status.complete",
    error: "task.status.error",
    removed: "task.status.removed",
  };

  return t(labels[props.status]);
});

const badgeType = computed(() => {
  if (props.status === "active") {
    return "success";
  }
  if (props.status === "error") {
    return "error";
  }
  if (props.status === "complete") {
    return "info";
  }
  if (props.status === "paused") {
    return "warning";
  }
  return "default";
});
</script>

<template>
  <NTag :type="badgeType" size="small" round>{{ label }}</NTag>
</template>
