<script setup lang="ts">
import TaskActions from "./TaskActions.vue";
import TaskProgressCell from "./TaskProgressCell.vue";
import TaskStatusBadge from "./TaskStatusBadge.vue";
import { t } from "../../../i18n";
import type { DownloadTask } from "../../../types/tasks";

const props = defineProps<{
  tasks: DownloadTask[];
}>();

function formatTaskError(task: DownloadTask) {
  const code = task.errorCode ? t("task.errorCode", { code: task.errorCode }) : "";
  return `${code}${task.errorMessage || t("common.unknown")}`;
}

function formatSize(size: number) {
  if (size <= 0) {
    return "0 B";
  }

  const units = ["B", "KB", "MB", "GB", "TB"];
  let value = size;
  let unitIndex = 0;

  while (value >= 1024 && unitIndex < units.length - 1) {
    value /= 1024;
    unitIndex += 1;
  }

  return `${value.toFixed(value >= 10 || unitIndex === 0 ? 0 : 1)} ${units[unitIndex]}`;
}

function formatSizePair(task: DownloadTask) {
  if (task.totalLength <= 0) {
    return `${formatSize(task.completedLength)} / ${t("common.unknown")}`;
  }

  return `${formatSize(task.completedLength)} / ${formatSize(task.totalLength)}`;
}

function formatEta(task: DownloadTask) {
  if (task.downloadSpeed <= 0 || task.totalLength <= task.completedLength) {
    return "--";
  }

  const seconds = Math.ceil((task.totalLength - task.completedLength) / task.downloadSpeed);
  if (seconds < 60) {
    return `${seconds}s`;
  }

  const minutes = Math.floor(seconds / 60);
  const restSeconds = seconds % 60;
  return `${minutes}m ${restSeconds}s`;
}
</script>

<template>
  <section class="task-mobile-list">
    <article v-for="task in props.tasks" :key="task.id" class="task-card">
      <header class="task-card-header">
        <strong :title="task.fileName">{{ task.fileName }}</strong>
        <TaskStatusBadge :status="task.status" />
      </header>

      <p class="task-card-url" :title="task.url">{{ task.url }}</p>
      <p v-if="task.status === 'error'" class="task-card-error" :title="formatTaskError(task)">{{ formatTaskError(task) }}</p>

      <div class="task-card-progress">
        <TaskProgressCell :task="task" />
      </div>

      <dl class="task-card-meta">
        <div>
          <dt>{{ t("task.table.size") }}</dt>
          <dd>{{ formatSizePair(task) }}</dd>
        </div>
        <div>
          <dt>{{ t("task.table.speed") }}</dt>
          <dd>{{ formatSize(task.downloadSpeed) }}/s</dd>
        </div>
        <div>
          <dt>{{ t("task.table.eta") }}</dt>
          <dd>{{ formatEta(task) }}</dd>
        </div>
      </dl>

      <footer class="task-card-actions">
        <TaskActions :task="task" compact />
      </footer>
    </article>
  </section>
</template>

<style scoped>
.task-mobile-list {
  display: grid;
  gap: 14px;
  padding: 16px;
}

.task-card {
  min-width: 0;
  display: grid;
  gap: 12px;
  padding: 16px;
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: 16px;
  background: #1a1d1c;
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.18);
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
  color: #83958e;
  font-size: 12px;
}

.task-card-meta dd {
  margin: 0;
  color: #d7dfd8;
  font-size: 13px;
  line-height: 1.4;
  word-break: break-word;
}

.task-card-actions {
  min-width: 0;
}
</style>
