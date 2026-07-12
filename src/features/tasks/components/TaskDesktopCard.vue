<script setup lang="ts">
import TaskActionsContainer from "./TaskActionsContainer.vue";
import TaskCardHeader from "./TaskCardHeader.vue";
import TaskErrorMessage from "./TaskErrorMessage.vue";
import TaskMetaItems from "./TaskMetaItems.vue";
import TaskProgressCell from "./TaskProgressCell.vue";
import { t } from "../../../i18n";
import type { DownloadTask } from "../../../types/tasks";

const props = defineProps<{
  task: DownloadTask;
}>();
</script>

<template>
  <article class="task-desktop-card task-card-main" data-test="task-desktop-card">
      <TaskCardHeader :task="props.task" variant="desktop">
        <template #actions>
          <div :aria-label="t('task.table.actions')">
            <TaskActionsContainer :task="props.task" variant="icon-pill" />
          </div>
        </template>
      </TaskCardHeader>

      <section class="task-card-body">
        <TaskProgressCell class="task-card-progress" :task="props.task" :show-label="false" variant="card" />
        <TaskMetaItems :task="props.task" variant="inline" />
      </section>

      <TaskErrorMessage :task="props.task" variant="single-line" />
  </article>
</template>

<style scoped>
.task-desktop-card {
  width: 100%;
  min-width: 0;
  min-height: 86px;
  background: transparent;
}

.task-card-main {
  min-width: 0;
  display: grid;
  grid-template-rows: auto auto auto;
  gap: 5px;
  padding: 14px;
}

.task-desktop-card :deep(.task-card-actions) {
  opacity: 0.42;
  transition: opacity var(--app-transition-fast);
}

.task-desktop-card:hover :deep(.task-card-actions),
.task-desktop-card:focus-within :deep(.task-card-actions) {
  opacity: 1;
}

.task-card-body {
  min-width: 0;
  display: grid;
  gap: 7px;
}

.task-card-progress {
  min-width: 0;
}
</style>
