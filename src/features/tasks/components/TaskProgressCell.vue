<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from "vue";
import type { DownloadTask } from "../../../types/tasks";

const props = defineProps<{
  task: DownloadTask;
}>();

const SPEED_SMOOTHING_ALPHA = 0.3;
const MAX_PREDICTION_SECONDS = 0.65;
const FORWARD_EASE_RATIO = 0.18;
const BACKWARD_EASE_RATIO = 0.08;
const SNAP_BYTE_DELTA = 0.5;

const initialCompletedLength = clampCompletedLength(props.task.completedLength, props.task.totalLength);
const displayCompletedLength = ref(initialCompletedLength);
const displayPercentage = computed(() => {
  if (props.task.totalLength <= 0) {
    return 0;
  }

  return clampPercentage((displayCompletedLength.value / props.task.totalLength) * 100);
});

const progressFillStyle = computed(() => ({
  transform: `scaleX(${displayPercentage.value / 100})`,
}));

let animationFrame = 0;
let anchorCompletedLength = initialCompletedLength;
let anchorUpdatedAt = performance.now();
let smoothedSpeed = normalizedSpeed(props.task.downloadSpeed, props.task.status);

watch(
  () =>
    [
      props.task.id,
      props.task.gid ?? "",
      props.task.status,
      props.task.totalLength,
      props.task.completedLength,
      props.task.downloadSpeed,
    ] as const,
  ([taskId, gid, status, totalLength, completedLength, downloadSpeed], previousSnapshot) => {
    const nextCompletedLength = clampCompletedLength(completedLength, totalLength);
    const shouldReset =
      !previousSnapshot ||
      taskId !== previousSnapshot[0] ||
      gid !== previousSnapshot[1] ||
      totalLength !== previousSnapshot[3] ||
      totalLength <= 0;

    anchorCompletedLength = nextCompletedLength;
    anchorUpdatedAt = performance.now();
    updateSmoothedSpeed(downloadSpeed, status, shouldReset);

    if (shouldReset) {
      displayCompletedLength.value = nextCompletedLength;
    }

    ensureAnimationFrame();
  },
  { immediate: true },
);

onBeforeUnmount(() => {
  cancelAnimationFrame(animationFrame);
});

function ensureAnimationFrame() {
  if (animationFrame) {
    return;
  }

  animationFrame = requestAnimationFrame(tickDisplayProgress);
}

function tickDisplayProgress(now: number) {
  animationFrame = 0;

  const target = displayCompletedTarget(now);
  const delta = target - displayCompletedLength.value;
  const shouldKeepPredicting = isPredictableTask() && smoothedSpeed > 0;

  if (Math.abs(delta) <= SNAP_BYTE_DELTA) {
    displayCompletedLength.value = target;
  } else {
    displayCompletedLength.value = clampCompletedLength(
      displayCompletedLength.value + delta * (delta > 0 ? FORWARD_EASE_RATIO : BACKWARD_EASE_RATIO),
      props.task.totalLength,
    );
  }

  if (shouldKeepPredicting || Math.abs(delta) > SNAP_BYTE_DELTA) {
    animationFrame = requestAnimationFrame(tickDisplayProgress);
  }
}

function displayCompletedTarget(now: number) {
  if (!isPredictableTask() || smoothedSpeed <= 0) {
    return anchorCompletedLength;
  }

  const elapsedSeconds = Math.min((now - anchorUpdatedAt) / 1000, MAX_PREDICTION_SECONDS);
  const predictedCompletedLength = anchorCompletedLength + smoothedSpeed * elapsedSeconds;

  if (anchorCompletedLength >= props.task.totalLength) {
    return props.task.totalLength;
  }

  return Math.min(
    clampCompletedLength(predictedCompletedLength, props.task.totalLength),
    props.task.totalLength * 0.9999,
  );
}

function updateSmoothedSpeed(downloadSpeed: number, status: DownloadTask["status"], reset: boolean) {
  const nextSpeed = normalizedSpeed(downloadSpeed, status);
  if (reset || nextSpeed <= 0 || smoothedSpeed <= 0) {
    smoothedSpeed = nextSpeed;
    return;
  }

  smoothedSpeed = smoothedSpeed * (1 - SPEED_SMOOTHING_ALPHA) + nextSpeed * SPEED_SMOOTHING_ALPHA;
}

function normalizedSpeed(downloadSpeed: number, status: DownloadTask["status"]) {
  return isLiveStatus(status) ? Math.max(0, downloadSpeed) : 0;
}

function isPredictableTask() {
  return isLiveStatus(props.task.status) && props.task.totalLength > 0 && anchorCompletedLength < props.task.totalLength;
}

function isLiveStatus(status: DownloadTask["status"]) {
  return status === "pending" || status === "active";
}

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
  will-change: transform;
}

small {
  color: #a8bab3;
  font-variant-numeric: tabular-nums;
}
</style>
