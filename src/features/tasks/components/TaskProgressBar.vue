<script setup lang="ts">
import { computed } from "vue";

const props = withDefaults(
  defineProps<{
    percentage: number;
    variant?: "compact" | "card";
    tone?: "default" | "complete" | "empty";
  }>(),
  {
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
const progressPercentage = computed(() => (props.tone === "empty" ? 0 : normalizedPercentage.value));
const progressScale = computed(() => progressPercentage.value / 100);
const progressStyle = computed(() => ({
  "--task-progress-scale": String(progressScale.value),
}));
</script>

<template>
  <div
    class="task-progress-bar"
    :class="[`task-progress-bar--${props.variant}`, `task-progress-bar--${props.tone}`]"
    :style="progressStyle"
    role="progressbar"
    aria-valuemin="0"
    aria-valuemax="100"
    :aria-valuenow="progressPercentage"
  >
    <div class="task-progress-bar__fill" aria-hidden="true" />
  </div>
</template>

<style scoped>
.task-progress-bar {
  min-width: 0;
  overflow: hidden;
  border-radius: 999px;
  background: color-mix(in srgb, var(--app-task-progress-rail) 58%, transparent);
}

.task-progress-bar--compact {
  height: 5px;
}

.task-progress-bar--card {
  height: 4px;
}

.task-progress-bar--empty {
  background: repeating-linear-gradient(
    90deg,
    color-mix(in srgb, var(--app-text-secondary) 18%, transparent) 0,
    color-mix(in srgb, var(--app-text-secondary) 18%, transparent) 8px,
    transparent 8px,
    transparent 14px
  );
}

.task-progress-bar__fill {
  width: 100%;
  height: 100%;
  transform: scaleX(var(--task-progress-scale));
  transform-origin: left center;
  transition: transform var(--app-transition-progress);
  border-radius: inherit;
}

.task-progress-bar--default .task-progress-bar__fill {
  background: linear-gradient(
    90deg,
    color-mix(in srgb, var(--app-text-accent-soft) 56%, var(--app-color-surface-elevated)),
    color-mix(in srgb, var(--app-text-accent-soft) 76%, var(--app-color-surface-elevated))
  );
}

.task-progress-bar--complete .task-progress-bar__fill {
  background: linear-gradient(
    90deg,
    color-mix(in srgb, var(--app-text-accent-soft) 72%, var(--app-color-surface-elevated)),
    var(--app-text-accent-soft)
  );
}
</style>
