<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from "vue";
import type { DownloadTask } from "../../../types/tasks";

const props = defineProps<{
  task: DownloadTask;
}>();

const PROGRESS_ANIMATION_MS = 360;

const completedLength = computed(() => clampCompletedLength(props.task.completedLength));

const targetPercentage = computed(() => {
  if (props.task.totalLength <= 0) {
    return 0;
  }

  return Math.min(100, (completedLength.value / props.task.totalLength) * 100);
});

const displayPercentage = ref(targetPercentage.value);

const progressFillStyle = computed(() => ({
  transform: `scaleX(${displayPercentage.value / 100})`,
}));

let animationFrame = 0;

watch(
  () =>
    [
      props.task.id,
      props.task.gid ?? "",
      props.task.totalLength,
      targetPercentage.value,
    ] as const,
  ([taskId, gid, totalLength, nextPercentage], previousSnapshot) => {
    const shouldReset =
      !previousSnapshot ||
      taskId !== previousSnapshot[0] ||
      gid !== previousSnapshot[1] ||
      totalLength !== previousSnapshot[2] ||
      totalLength <= 0;

    if (shouldReset) {
      setDisplayPercentage(nextPercentage);
      return;
    }

    animateDisplayPercentage(nextPercentage);
  },
  { immediate: true },
);

onBeforeUnmount(() => {
  cancelAnimationFrame(animationFrame);
});

function setDisplayPercentage(value: number) {
  cancelAnimationFrame(animationFrame);
  displayPercentage.value = clampPercentage(value);
}

function animateDisplayPercentage(value: number) {
  cancelAnimationFrame(animationFrame);

  const from = displayPercentage.value;
  const to = clampPercentage(value);
  if (Math.abs(to - from) < 0.01) {
    displayPercentage.value = to;
    return;
  }

  const startedAt = performance.now();
  const tick = (now: number) => {
    const progress = Math.min(1, (now - startedAt) / PROGRESS_ANIMATION_MS);
    displayPercentage.value = from + (to - from) * easeOutCubic(progress);

    if (progress < 1) {
      animationFrame = requestAnimationFrame(tick);
    } else {
      displayPercentage.value = to;
    }
  };

  animationFrame = requestAnimationFrame(tick);
}

function easeOutCubic(value: number) {
  return 1 - Math.pow(1 - value, 3);
}

function clampPercentage(value: number) {
  return Math.max(0, Math.min(100, value));
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
