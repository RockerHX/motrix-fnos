<script setup lang="ts">
import { computed } from "vue";
import { NTag } from "naive-ui";
import type { DownloadTask } from "../../../types/tasks";
import { deriveTaskDisplayStatus, formatTaskStatusLabel } from "../utils/taskFormat";

const props = defineProps<{
  task: DownloadTask;
}>();

const displayStatus = computed(() => deriveTaskDisplayStatus(props.task));
const label = computed(() => formatTaskStatusLabel(props.task));

const badgeType = computed(() => {
  if (displayStatus.value === "active") {
    return "success";
  }
  if (displayStatus.value === "error") {
    return "error";
  }
  if (displayStatus.value === "complete") {
    return "info";
  }
  if (displayStatus.value === "paused" || displayStatus.value === "confirming") {
    return "warning";
  }
  return "default";
});
</script>

<template>
  <NTag :type="badgeType" size="small" round>{{ label }}</NTag>
</template>
