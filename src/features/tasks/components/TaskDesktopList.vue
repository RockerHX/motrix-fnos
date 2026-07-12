<script setup lang="ts">
import { NList, NListItem } from "naive-ui";
import TaskDesktopCard from "./TaskDesktopCard.vue";
import type { DownloadTask } from "../../../types/tasks";

const props = defineProps<{
  tasks: DownloadTask[];
}>();
</script>

<template>
  <section class="task-desktop-list" data-test="task-desktop-list">
    <NList class="task-desktop-list-scroll" hoverable show-divider>
      <NListItem v-for="task in props.tasks" :key="task.id">
        <TaskDesktopCard :task="task" />
      </NListItem>
    </NList>
  </section>
</template>

<style scoped>
.task-desktop-list {
  min-width: 0;
  min-height: 0;
  height: 100%;
  overflow-x: hidden;
  overflow-y: auto;
  padding: 16px;
  scrollbar-gutter: stable;
}

.task-desktop-list-scroll {
  border: 1px solid var(--app-color-border-subtle);
  border-bottom: 0;
  border-radius: 0;
  background: transparent;
}

.task-desktop-list-scroll :deep(.n-list-item) {
  padding: 0;
  border-bottom: 1px solid var(--app-color-border-subtle) !important;
  border-radius: 0 !important;
}

.task-desktop-list-scroll :deep(.n-list-item:hover),
.task-desktop-list-scroll :deep(.n-list-item:focus),
.task-desktop-list-scroll :deep(.n-list-item:focus-within) {
  border-radius: 0 !important;
}

@media (max-width: 1023px) {
  .task-desktop-list {
    padding: 16px;
  }
}
</style>
