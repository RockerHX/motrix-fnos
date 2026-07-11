<script setup lang="ts">
import { computed } from "vue";
import { NProgress } from "naive-ui";
import type { CSSProperties } from "vue";

const props = withDefaults(
  defineProps<{
    percentage: number;
    transitionMs?: number;
    variant?: "compact" | "card";
    tone?: "default" | "complete" | "empty";
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
const progressPercentage = computed(() => (props.tone === "empty" ? 0 : normalizedPercentage.value));
const progressHeight = computed(() => (props.variant === "card" ? 4 : 5));
const progressColor = computed(() =>
  props.tone === "complete"
    ? {
        stops: [
          "color-mix(in srgb, var(--app-text-accent-soft) 72%, var(--app-color-surface-elevated))",
          "var(--app-text-accent-soft)",
        ],
      }
    : {
        stops: [
          "color-mix(in srgb, var(--app-text-accent-soft) 56%, var(--app-color-surface-elevated))",
          "color-mix(in srgb, var(--app-text-accent-soft) 76%, var(--app-color-surface-elevated))",
        ],
      },
);
const railStyle = computed<CSSProperties>(() =>
  props.tone === "empty"
    ? {
        background:
          "repeating-linear-gradient(90deg, color-mix(in srgb, var(--app-text-secondary) 18%, transparent) 0, color-mix(in srgb, var(--app-text-secondary) 18%, transparent) 8px, transparent 8px, transparent 14px)",
      }
    : {},
);
const progressStyle = computed(() => ({
  "--task-progress-transition-ms": `${props.transitionMs}ms`,
}));
</script>

<template>
  <div
    class="task-progress-bar"
    :class="[`task-progress-bar--${props.variant}`, `task-progress-bar--${props.tone}`]"
    :style="progressStyle"
  >
    <NProgress
      type="line"
      :percentage="progressPercentage"
      :height="progressHeight"
      :border-radius="progressHeight"
      :fill-border-radius="progressHeight"
      :color="progressColor"
      rail-color="color-mix(in srgb, var(--app-task-progress-track) 58%, transparent)"
      :rail-style="railStyle"
      :show-indicator="false"
    />
  </div>
</template>

<style scoped>
.task-progress-bar {
  min-width: 0;
}

.task-progress-bar :deep(.n-progress-graph-line-fill) {
  transition-duration: var(--task-progress-transition-ms);
  transition-timing-function: ease-out;
  will-change: max-width;
}
</style>
