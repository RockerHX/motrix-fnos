<script setup lang="ts">
import TaskActionsContainer from "./TaskActionsContainer.vue";
import TaskProgressCell from "./TaskProgressCell.vue";
import TaskStatusBadge from "./TaskStatusBadge.vue";
import { t } from "../../../i18n";
import { formatTaskError, formatTaskEta, formatTaskSize, formatTaskSizePair } from "../utils/taskFormat";
import type { DownloadTask } from "../../../types/tasks";

const props = defineProps<{
  task: DownloadTask;
}>();
</script>

<template>
  <article class="task-desktop-card" :class="`task-desktop-card--${props.task.status}`" data-test="task-desktop-card">
    <div class="task-card-status-rail" aria-hidden="true" data-test="task-card-status-rail" />

    <div class="task-card-main">
      <header class="task-card-header">
        <div class="task-card-title-group">
          <strong class="task-card-title" :title="props.task.fileName">{{ props.task.fileName }}</strong>
          <TaskStatusBadge :task="props.task" />
        </div>
        <aside class="task-card-actions" :aria-label="t('task.table.actions')">
          <TaskActionsContainer :task="props.task" variant="icon-pill" />
        </aside>
      </header>

      <section class="task-card-body">
        <TaskProgressCell class="task-card-progress" :task="props.task" :show-label="false" variant="card" />
        <footer class="task-card-meta">
          <span class="task-card-size">{{ formatTaskSizePair(props.task) }}</span>
          <span>{{ t("task.table.speed") }} {{ formatTaskSize(props.task.downloadSpeed) }}/s</span>
          <span>{{ t("task.table.eta") }} {{ formatTaskEta(props.task) }}</span>
        </footer>
      </section>

      <div class="task-card-error-slot" data-test="task-card-error-slot">
        <p v-if="props.task.status === 'error'" class="task-card-error" :title="formatTaskError(props.task)">
          {{ formatTaskError(props.task) }}
        </p>
      </div>
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

.task-card-header {
  min-width: 0;
  display: grid;
  grid-template-columns: minmax(0, 1fr) max-content;
  align-items: center;
  gap: 10px;
}

.task-card-title-group {
  min-width: 0;
  display: flex;
  align-items: center;
  gap: 6px;
}

.task-card-title-group :deep(.n-tag) {
  --n-height: 18px;
  --n-font-size: 10px;
  --n-border-radius: var(--app-radius-pill);
  opacity: 0.62;
}

.task-card-title {
  min-width: 0;
  overflow: hidden;
  color: var(--app-text-strong);
  font-size: 14px;
  font-weight: 500;
  line-height: 1.25;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.task-card-actions {
  min-width: 0;
  display: flex;
  justify-content: flex-end;
  opacity: 0.42;
  transition: opacity var(--app-transition-fast);
}

.task-desktop-card:hover .task-card-actions,
.task-desktop-card:focus-within .task-card-actions {
  opacity: 1;
}

.task-card-body {
  min-width: 0;
  display: grid;
  grid-template-columns: minmax(180px, min(58vw, 680px)) max-content;
  align-items: center;
  gap: 14px;
}

.task-card-progress {
  min-width: 0;
  max-width: 680px;
}

.task-card-meta {
  min-width: 0;
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 12px;
  color: var(--app-text-muted);
  font-size: 11px;
  line-height: 1.35;
  white-space: nowrap;
  font-variant-numeric: tabular-nums;
}

.task-card-size {
  color: var(--app-text-secondary);
}

.task-card-error-slot {
  min-width: 0;
  min-height: 0;
}

.task-card-error {
  overflow: hidden;
  margin: 0;
  color: var(--app-text-danger);
  font-size: 11px;
  line-height: 1.35;
  text-overflow: ellipsis;
  white-space: nowrap;
}

@media (max-width: 900px) {
  .task-card-body {
    grid-template-columns: minmax(0, 1fr);
  }

  .task-card-meta {
    justify-content: flex-start;
    flex-wrap: wrap;
    gap: 8px 12px;
  }
}
</style>
