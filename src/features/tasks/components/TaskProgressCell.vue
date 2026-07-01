<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from "vue";
import type { DownloadTask } from "../../../types/tasks";

const props = defineProps<{
  task: DownloadTask;
}>();

const estimatedCompletedLength = ref(clampCompletedLength(props.task.completedLength));
let animationFrame = 0;
let sampleStartedAt = performance.now();
let sampleCompletedLength = props.task.completedLength;

const displayPercentage = computed(() => {
  if (props.task.totalLength <= 0) {
    return 0;
  }

  return Math.min(100, (estimatedCompletedLength.value / props.task.totalLength) * 100);
});

const progressFillStyle = computed(() => ({
  transform: `scaleX(${displayPercentage.value / 100})`,
}));

watch(
  () => [
    props.task.completedLength,
    props.task.totalLength,
    props.task.downloadSpeed,
    props.task.status,
  ],
  () => {
    sampleStartedAt = performance.now();
    sampleCompletedLength = clampCompletedLength(props.task.completedLength);
    estimatedCompletedLength.value = sampleCompletedLength;
    restartProgressLoop();
  },
  { immediate: true },
);

onBeforeUnmount(() => {
  cancelAnimationFrame(animationFrame);
});

function restartProgressLoop() {
  cancelAnimationFrame(animationFrame);

  if (!canEstimateProgress()) {
    estimatedCompletedLength.value = clampCompletedLength(props.task.completedLength);
    return;
  }

  animationFrame = requestAnimationFrame(updateEstimatedProgress);
}

function updateEstimatedProgress(now: number) {
  if (!canEstimateProgress()) {
    estimatedCompletedLength.value = clampCompletedLength(props.task.completedLength);
    return;
  }

  const elapsedSeconds = Math.max(0, (now - sampleStartedAt) / 1000);
  estimatedCompletedLength.value = clampCompletedLength(
    sampleCompletedLength + props.task.downloadSpeed * elapsedSeconds,
  );

  if (estimatedCompletedLength.value < props.task.totalLength) {
    animationFrame = requestAnimationFrame(updateEstimatedProgress);
  }
}

function canEstimateProgress() {
  return (
    props.task.status === "active" &&
    props.task.totalLength > 0 &&
    props.task.downloadSpeed > 0 &&
    props.task.completedLength < props.task.totalLength
  );
}

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
