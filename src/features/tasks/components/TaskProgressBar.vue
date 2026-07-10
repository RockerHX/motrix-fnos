<script setup lang="ts">
import { computed } from "vue";

const props = withDefaults(
  defineProps<{
    percentage: number;
    transitionMs?: number;
    variant?: "compact" | "card";
    tone?: "default" | "complete";
  }>(),
  {
    transitionMs: 360,
    variant: "compact",
    tone: "default",
  },
);

const normalizedPercentage = computed(() => {
  if (!Number.isFinite(props.percentage)) {
    return 0;
  }

  return Math.max(0, Math.min(100, props.percentage));
});
const progressFillStyle = computed(() => ({
  transform: `scaleX(${normalizedPercentage.value / 100})`,
  transitionDuration: `${props.transitionMs}ms`,
}));
</script>

<template>
  <div class="task-progress-bar" :class="[`task-progress-bar--${props.variant}`, `task-progress-bar--${props.tone}`]">
    <div class="progress-track" aria-hidden="true">
      <div class="progress-fill" :style="progressFillStyle" />
    </div>
  </div>
</template>

<style scoped>
.task-progress-bar {
  min-width: 0;
}

.progress-track {
  overflow: hidden;
  height: 8px;
  border-radius: var(--app-radius-pill);
  background: var(--app-task-progress-track);
}

.task-progress-bar--card .progress-track {
  height: 12px;
}

.progress-fill {
  width: 100%;
  height: 100%;
  border-radius: inherit;
  background: linear-gradient(90deg, #78c8f0, var(--app-text-accent-soft));
  transform-origin: left center;
  transition-property: transform;
  transition-timing-function: ease-out;
  will-change: transform;
}

.task-progress-bar--complete .progress-fill {
  background: linear-gradient(
    90deg,
    color-mix(in srgb, var(--app-text-accent-soft) 72%, var(--app-color-surface-elevated)),
    var(--app-text-accent-soft)
  );
}
</style>
