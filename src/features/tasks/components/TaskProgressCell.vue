<script setup lang="ts">
import { computed, ref, watch } from "vue";
import type { DownloadTask } from "../../../types/tasks";

const props = defineProps<{
  task: DownloadTask;
}>();

const TRANSITION_MS = 360;
const displayCompletedLength = ref(clampCompletedLength(props.task.completedLength, props.task.totalLength));
const displayPercentage = computed(() => {
  if (props.task.totalLength <= 0) {
    return 0;
  }

  return clampPercentage((displayCompletedLength.value / props.task.totalLength) * 100);
});
const progressFillStyle = computed(() => ({
  transform: `scaleX(${displayPercentage.value / 100})`,
  transitionDuration: `${TRANSITION_MS}ms`,
}));

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

    if (status === "complete" && totalLength > 0) {
      displayCompletedLength.value = totalLength;
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
    <div class="progress-track" aria-hidden="true">
      <div class="progress-fill" :style="progressFillStyle" />
    </div>
    <small>{{ displayPercentage.toFixed(2) }}%</small>
  </div>
</template>

<style scoped>
.task-progress-cell {
  min-width: 0;
  display: grid;
  gap: 6px;
}

.progress-track {
  overflow: hidden;
  height: 8px;
  border-radius: 999px;
  background: rgba(255, 255, 255, 0.14);
}

.progress-fill {
  width: 100%;
  height: 100%;
  border-radius: inherit;
  background: linear-gradient(90deg, #78c8f0, #66d89b);
  transform-origin: left center;
  transition-property: transform;
  transition-timing-function: ease-out;
  will-change: transform;
}

small {
  color: #a8bab3;
  font-variant-numeric: tabular-nums;
}
</style>
