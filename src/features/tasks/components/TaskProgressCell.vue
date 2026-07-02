<script setup lang="ts">
import { computed } from "vue";
import type { DownloadTask } from "../../../types/tasks";

const props = defineProps<{
  task: DownloadTask;
}>();

const completedLength = computed(() => clampCompletedLength(props.task.completedLength));

const displayPercentage = computed(() => {
  if (props.task.totalLength <= 0) {
    return 0;
  }

  return Math.min(100, (completedLength.value / props.task.totalLength) * 100);
});

const progressFillStyle = computed(() => ({
  transform: `scaleX(${displayPercentage.value / 100})`,
}));

function clampCompletedLength(value: number) {
  if (props.task.totalLength <= 0) {
    return 0;
  }

  return Math.max(0, Math.min(props.task.totalLength, value));
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
  will-change: transform;
}

small {
  color: #a8bab3;
  font-variant-numeric: tabular-nums;
}
</style>
