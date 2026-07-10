<script setup lang="ts">
import { computed, ref, watch } from "vue";
import TaskProgressBar from "./TaskProgressBar.vue";
import type { DownloadTask } from "../../../types/tasks";

const props = withDefaults(
  defineProps<{
    task: DownloadTask;
    showLabel?: boolean;
    variant?: "compact" | "card";
  }>(),
  {
    showLabel: true,
    variant: "compact",
  },
);

const TRANSITION_MS = 360;
const displayCompletedLength = ref(clampCompletedLength(props.task.completedLength, props.task.totalLength));
const displayPercentage = computed(() => {
  if (props.task.totalLength <= 0) {
    return 0;
  }

  return clampPercentage((displayCompletedLength.value / props.task.totalLength) * 100);
});
const progressTone = computed(() => (props.task.status === "complete" ? "complete" : "default"));
watch(
  () =>
    [
      props.task.id,
      props.task.gid ?? "",
      props.task.status,
      props.task.totalLength,
      props.task.completedLength,
    ] as const,
  ([taskId, gid, status, totalLength, completedLength], previousSnapshot) => {
    const nextCompletedLength = clampCompletedLength(completedLength, totalLength);
    if (status === "complete" && totalLength > 0) {
      displayCompletedLength.value = totalLength;
      return;
    }

    const shouldReset =
      !previousSnapshot ||
      taskId !== previousSnapshot[0] ||
      gid !== previousSnapshot[1] ||
      totalLength !== previousSnapshot[3] ||
      totalLength <= 0;

    if (shouldReset) {
      displayCompletedLength.value = nextCompletedLength;
      return;
    }

    displayCompletedLength.value = Math.max(displayCompletedLength.value, nextCompletedLength);
  },
  { immediate: true },
);

function clampPercentage(value: number) {
  return Math.max(0, Math.min(100, value));
}

function clampCompletedLength(value: number, totalLength: number) {
  if (totalLength <= 0) {
    return 0;
  }

  return Math.max(0, Math.min(totalLength, value));
}
</script>

<template>
  <div class="task-progress-cell">
    <TaskProgressBar
      :percentage="displayPercentage"
      :transition-ms="TRANSITION_MS"
      :variant="props.variant"
      :tone="progressTone"
    />
    <small v-if="props.showLabel">{{ displayPercentage.toFixed(2) }}%</small>
  </div>
</template>

<style scoped>
.task-progress-cell {
  min-width: 0;
  display: grid;
  gap: 6px;
}

small {
  color: #a8bab3;
  font-variant-numeric: tabular-nums;
}
</style>
