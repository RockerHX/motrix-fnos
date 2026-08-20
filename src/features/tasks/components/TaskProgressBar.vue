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

<style scoped src="./TaskProgressBar.css"></style>
