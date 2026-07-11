<script setup lang="ts">
import { t } from "../../../i18n";
import { formatTaskEta, formatTaskSize, formatTaskSizePair } from "../utils/taskFormat";
import type { DownloadTask } from "../../../types/tasks";

const props = withDefaults(
  defineProps<{
    task: DownloadTask;
    variant?: "inline" | "grid";
  }>(),
  {
    variant: "inline",
  },
);
</script>

<template>
  <footer v-if="props.variant === 'inline'" class="task-card-meta task-card-meta--inline">
    <span class="task-card-size">{{ formatTaskSizePair(props.task) }}</span>
    <span>{{ t("task.table.speed") }} {{ formatTaskSize(props.task.downloadSpeed) }}/s</span>
    <span>{{ t("task.table.eta") }} {{ formatTaskEta(props.task) }}</span>
  </footer>

  <dl v-else class="task-card-meta task-card-meta--grid">
    <div>
      <dt>{{ t("task.table.size") }}</dt>
      <dd>{{ formatTaskSizePair(props.task) }}</dd>
    </div>
    <div>
      <dt>{{ t("task.table.speed") }}</dt>
      <dd>{{ formatTaskSize(props.task.downloadSpeed) }}/s</dd>
    </div>
    <div>
      <dt>{{ t("task.table.eta") }}</dt>
      <dd>{{ formatTaskEta(props.task) }}</dd>
    </div>
  </dl>
</template>

<style scoped>
.task-card-meta {
  min-width: 0;
  margin: 0;
  font-variant-numeric: tabular-nums;
}

.task-card-meta--inline {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 12px;
  color: var(--app-text-muted);
  font-size: 11px;
  line-height: 1.35;
  white-space: nowrap;
}

.task-card-size {
  color: var(--app-text-secondary);
}

.task-card-meta--grid {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 10px;
}

.task-card-meta--grid div {
  min-width: 0;
}

.task-card-meta--grid dt {
  margin-bottom: 6px;
  color: var(--app-text-dim);
  font-size: 12px;
}

.task-card-meta--grid dd {
  overflow: hidden;
  margin: 0;
  color: var(--app-text-secondary);
  font-size: 13px;
  line-height: 1.4;
  text-overflow: ellipsis;
  white-space: nowrap;
}

@media (max-width: 900px) {
  .task-card-meta--inline {
    justify-content: flex-start;
    flex-wrap: wrap;
    gap: 8px 12px;
  }
}

@media (max-width: 767px) {
  .task-card-meta--grid {
    gap: 8px;
  }

  .task-card-meta--grid dt {
    margin-bottom: 4px;
    font-size: 11px;
  }

  .task-card-meta--grid dd {
    font-size: 12px;
    line-height: 1.5;
  }
}
</style>
