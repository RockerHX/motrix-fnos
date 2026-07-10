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
    <div class="task-card-rail" aria-hidden="true">
      <span class="task-card-grip">⠿</span>
    </div>

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

      <div class="task-card-error-slot" data-test="task-card-error-slot">
        <p v-if="props.task.status === 'error'" class="task-card-error" :title="formatTaskError(props.task)">
          {{ formatTaskError(props.task) }}
        </p>
      </div>

      <TaskProgressCell class="task-card-progress" :task="props.task" :show-label="false" variant="card" />

      <footer class="task-card-meta">
        <span class="task-card-size">{{ formatTaskSizePair(props.task) }}</span>
        <span>{{ t("task.table.speed") }} {{ formatTaskSize(props.task.downloadSpeed) }}/s</span>
        <span>{{ t("task.table.eta") }} {{ formatTaskEta(props.task) }}</span>
      </footer>
    </div>
  </article>
</template>

<style scoped>
.task-desktop-card {
  min-width: 0;
  min-height: 214px;
  display: grid;
  grid-template-columns: 52px minmax(0, 1fr);
  overflow: hidden;
  border: 1px solid var(--app-color-border-subtle);
  border-radius: var(--app-task-card-radius);
  background: var(--app-color-surface-elevated);
  box-shadow: var(--app-shadow-card);
}

.task-card-rail {
  display: grid;
  place-items: center;
  border-right: 1px solid var(--app-color-border-subtle);
  background: var(--app-color-card-overlay-subtle);
  color: var(--app-text-dim);
}

.task-card-grip {
  transform: rotate(90deg);
  font-size: 20px;
  letter-spacing: 2px;
  line-height: 1;
}

.task-card-main {
  min-width: 0;
  display: grid;
  grid-template-rows: auto auto minmax(0, 1fr) auto;
  gap: 18px;
  padding: 34px 26px 26px;
}

.task-card-header {
  min-width: 0;
  display: grid;
  grid-template-columns: minmax(0, 1fr) max-content;
  align-items: start;
  gap: 24px;
}

.task-card-title-group {
  min-width: 0;
  display: flex;
  align-items: center;
  gap: 14px;
}

.task-card-title {
  min-width: 0;
  overflow: hidden;
  color: var(--app-text-strong);
  font-size: 28px;
  font-weight: 500;
  line-height: 1.25;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.task-card-actions {
  min-width: 0;
  display: flex;
  justify-content: flex-end;
}

.task-card-error-slot {
  min-height: 20px;
  min-width: 0;
}

.task-card-error {
  overflow: hidden;
  margin: 0;
  color: var(--app-text-danger);
  font-size: 13px;
  line-height: 1.5;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.task-card-progress {
  align-self: center;
}

.task-card-meta {
  min-width: 0;
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 20px;
  color: var(--app-text-secondary);
  font-size: 18px;
  line-height: 1.4;
  font-variant-numeric: tabular-nums;
}

.task-card-size {
  margin-right: auto;
}

@media (max-width: 1180px) {
  .task-card-header {
    grid-template-columns: minmax(0, 1fr);
  }

  .task-card-actions {
    max-width: 100%;
    justify-content: flex-start;
  }

  .task-card-meta {
    align-items: flex-start;
    flex-direction: column;
    gap: 6px;
    font-size: 15px;
  }

  .task-card-size {
    margin-right: 0;
  }
}
</style>
