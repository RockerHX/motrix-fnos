<script setup lang="ts">
import { computed } from "vue";
import { t } from "../../../i18n";
import { formatTaskProgress, getTaskProgressPercentage } from "../utils/taskFormat";
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

const displayPercentage = computed(() => getTaskProgressPercentage(props.task));
const progressLabel = computed(() => formatTaskProgress(props.task));
const progressTone = computed(() => {
  if (displayPercentage.value === null) {
    return "empty";
  }

  return props.task.status === "complete" ? "complete" : "default";
});
</script>

<template>
  <div class="task-progress-cell">
    <TaskProgressBar
      :percentage="displayPercentage"
      :variant="props.variant"
      :tone="progressTone"
      :aria-label="`${t('task.detail.progress')} ${props.task.fileName}`"
      :aria-valuetext="displayPercentage === null ? t('task.progress.unknown') : progressLabel"
    />
    <small v-if="props.showLabel">{{ progressLabel }}</small>
  </div>
</template>

<style scoped src="./TaskProgressCell.css"></style>
