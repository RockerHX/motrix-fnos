<script setup lang="ts">
import { computed } from "vue";
import type { DownloadTask } from "../../../types/tasks";

const props = defineProps<{
  task: DownloadTask;
}>();

const percentage = computed(() => {
  if (props.task.totalLength <= 0) {
    return 0;
  }

  return Math.min(100, Math.round((props.task.completedLength / props.task.totalLength) * 100));
});
</script>

<template>
  <div class="task-progress-cell">
    <n-progress type="line" :percentage="percentage" :height="8" :show-indicator="false" processing />
    <small>{{ percentage }}%</small>
  </div>
</template>

<style scoped>
.task-progress-cell {
  min-width: 0;
  display: grid;
  gap: 6px;
}

small {
  color: #a8bab3;
}
</style>
