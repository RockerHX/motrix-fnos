<script setup lang="ts">
import TaskActionsContainer from "./TaskActionsContainer.vue";
import TaskProgressCell from "./TaskProgressCell.vue";
import TaskStatusBadge from "./TaskStatusBadge.vue";
import { t } from "../../../i18n";
import { formatTaskError, formatTaskEta, formatTaskSize, formatTaskSizePair } from "../utils/taskFormat";
import type { DownloadTask } from "../../../types/tasks";

const props = defineProps<{
  tasks: DownloadTask[];
}>();

</script>

<template>
  <section class="task-mobile-list">
    <article v-for="task in props.tasks" :key="task.id" class="task-card">
      <header class="task-card-header">
        <strong :title="task.fileName">{{ task.fileName }}</strong>
        <TaskStatusBadge :task="task" />
      </header>

      <p class="task-card-url" :title="task.url">{{ task.url }}</p>
      <p v-if="task.status === 'error'" class="task-card-error" :title="formatTaskError(task)">{{ formatTaskError(task) }}</p>

      <div class="task-card-progress">
        <TaskProgressCell :task="task" />
      </div>

      <dl class="task-card-meta">
        <div>
          <dt>{{ t("task.table.size") }}</dt>
          <dd>{{ formatTaskSizePair(task) }}</dd>
        </div>
        <div>
          <dt>{{ t("task.table.speed") }}</dt>
          <dd>{{ formatTaskSize(task.downloadSpeed) }}/s</dd>
        </div>
        <div>
          <dt>{{ t("task.table.eta") }}</dt>
          <dd>{{ formatTaskEta(task) }}</dd>
        </div>
      </dl>

      <footer class="task-card-actions">
        <TaskActionsContainer :task="task" compact />
      </footer>
    </article>
  </section>
</template>

<style scoped>
.task-mobile-list {
  display: grid;
  gap: 14px;
  padding: 16px;
  padding-bottom: calc(116px + var(--app-safe-area-bottom));
}

.task-card {
  min-width: 0;
  display: grid;
  gap: 12px;
  padding: 16px;
  border: 1px solid var(--app-color-border-subtle);
  border-radius: var(--app-radius-lg);
  background: var(--app-color-surface-elevated);
  box-shadow: var(--app-shadow-card);
}

.task-card-header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 10px;
}

.task-card-header strong {
  min-width: 0;
  display: -webkit-box;
  overflow: hidden;
  color: #f1f6f1;
  font-size: 16px;
  line-height: 1.4;
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 2;
}

.task-card-url,
.task-card-error {
  margin: 0;
  display: -webkit-box;
  overflow: hidden;
  color: #8e9a91;
  font-size: 13px;
  line-height: 1.5;
  word-break: break-word;
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 2;
}

.task-card-error {
  color: #ff9b9b;
}

.task-card-progress {
  min-width: 0;
}

.task-card-meta {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 10px;
  margin: 0;
}

.task-card-meta div {
  min-width: 0;
}

.task-card-meta dt {
  margin-bottom: 6px;
  color: var(--app-text-dim);
  font-size: 12px;
}

.task-card-meta dd {
  overflow: hidden;
  margin: 0;
  color: var(--app-text-secondary);
  font-size: 13px;
  line-height: 1.4;
  font-variant-numeric: tabular-nums;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.task-card-actions {
  min-width: 0;
}

@media (max-width: 767px) {
  .task-mobile-list {
    gap: 12px;
    padding: 14px var(--app-mobile-page-gutter);
    padding-bottom: calc(112px + var(--app-safe-area-bottom));
  }

  .task-card {
    gap: 10px;
    padding: 14px;
    border-radius: var(--app-radius-xl);
    background: #181b19;
  }

  .task-card-header {
    gap: 8px;
  }

  .task-card-header strong {
    font-size: 15px;
    line-height: 1.35;
  }

  .task-card-url,
  .task-card-error,
  .task-card-meta dd {
    font-size: 12px;
    line-height: 1.5;
  }

  .task-card-meta {
    gap: 8px;
  }

  .task-card-meta dt {
    margin-bottom: 4px;
    font-size: 11px;
  }
}
</style>
