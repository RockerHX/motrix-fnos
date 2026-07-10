<script setup lang="ts">
import TaskDesktopCard from "./TaskDesktopCard.vue";
import type { DownloadTask } from "../../../types/tasks";

const props = defineProps<{
  tasks: DownloadTask[];
}>();
</script>

<template>
  <section class="task-desktop-list" data-test="task-desktop-list">
    <div class="task-desktop-list-scroll">
      <TaskDesktopCard v-for="task in props.tasks" :key="task.id" :task="task" />
    </div>
  </section>
</template>

<style scoped>
.task-desktop-list {
  min-width: 0;
  min-height: 0;
  height: 100%;
  overflow: hidden;
}

.task-desktop-list-scroll {
  height: 100%;
  min-height: 0;
  display: grid;
  align-content: start;
  gap: 14px;
  overflow-x: hidden;
  overflow-y: auto;
  padding: 24px var(--app-desktop-content-gutter-x) 32px;
  scrollbar-gutter: stable;
}

@media (max-width: 1023px) {
  .task-desktop-list-scroll {
    gap: 16px;
    padding: 22px;
  }
}
</style>
