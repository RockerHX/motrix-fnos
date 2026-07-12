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
  <article class="task-desktop-card" :class="`task-desktop-card--${props.task.status}`" data-test="task-desktop-card">
    <div class="task-card-status-rail" aria-hidden="true" data-test="task-card-status-rail" />

    <div class="task-card-main">
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
    </div>
  </article>
</template>

<style scoped>
.task-desktop-card {
  min-width: 0;
  min-height: 86px;
  display: grid;
  grid-template-columns: 4px minmax(0, 1fr);
  overflow: hidden;
  border: 1px solid var(--app-color-border-subtle);
  border-radius: var(--app-task-card-radius);
  background: var(--app-color-surface-elevated);
  box-shadow: var(--app-shadow-card);
  transition:
    border-color var(--app-transition-fast),
    background var(--app-transition-fast);
}

.task-desktop-card:hover,
.task-desktop-card:focus-within {
  border-color: color-mix(in srgb, var(--app-text-accent-soft) 30%, var(--app-color-border-subtle));
}

.task-card-status-rail {
  background: color-mix(in srgb, var(--app-text-secondary) 34%, transparent);
}

.task-desktop-card--active .task-card-status-rail,
.task-desktop-card--pending .task-card-status-rail {
  background: color-mix(in srgb, var(--app-text-accent-soft) 62%, transparent);
}

.task-desktop-card--complete .task-card-status-rail {
  background: color-mix(in srgb, var(--app-text-accent-soft) 52%, var(--app-color-surface-elevated));
}

.task-desktop-card--error .task-card-status-rail {
  background: var(--app-text-danger);
}

.task-card-main {
  min-width: 0;
  display: grid;
  grid-template-rows: auto auto auto;
  gap: 5px;
  padding: 10px 14px;
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
