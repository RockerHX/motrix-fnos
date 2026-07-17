<script setup lang="ts">
import TaskStatusBadge from "./TaskStatusBadge.vue";
import type { DownloadTask } from "../../../types/tasks";

const props = withDefaults(
  defineProps<{
    task: DownloadTask;
    variant?: "desktop" | "mobile";
  }>(),
  {
    variant: "desktop",
  },
);
</script>

<template>
  <header class="task-card-header" :class="`task-card-header--${props.variant}`">
    <div class="task-card-title-group">
      <strong class="task-card-title" :title="props.task.fileName">{{ props.task.fileName }}</strong>
      <TaskStatusBadge :task="props.task" />
    </div>
    <aside v-if="$slots.actions" class="task-card-actions">
      <slot name="actions" />
    </aside>
  </header>
</template>

<style scoped src="./TaskCardHeader.css"></style>
