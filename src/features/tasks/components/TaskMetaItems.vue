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
    <span class="task-card-size task-card-metric" data-test="task-size-metric">
      <span class="task-card-metric-value">{{ formatTaskSizePair(props.task) }}</span>
    </span>
    <span class="task-card-metrics-end" data-test="task-dynamic-metrics">
      <span
        class="task-card-metric task-card-metric--eta"
        data-test="task-eta-metric"
        :aria-label="`${t('task.table.eta')} ${formatTaskEta(props.task)}`"
      >
        <span class="task-card-metric-label">{{ t("task.table.eta") }}</span>
        <span class="task-card-metric-value">{{ formatTaskEta(props.task) }}</span>
      </span>
      <span
        class="task-card-metric task-card-metric--speed"
        data-test="task-speed-metric"
        :aria-label="`${t('task.table.speed')} ${formatTaskSize(props.task.downloadSpeed)}/s`"
      >
        <span class="task-card-metric-label">{{ t("task.table.speed") }}</span>
        <span class="task-card-metric-value">{{ formatTaskSize(props.task.downloadSpeed) }}/s</span>
      </span>
    </span>
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
  width: 100%;
  display: grid;
  grid-template-columns: minmax(150px, 1fr) max-content;
  align-items: center;
  gap: 16px;
  color: var(--app-text-muted);
  font-size: 11px;
  line-height: 1.35;
  white-space: nowrap;
}

.task-card-metric {
  min-width: 0;
}

.task-card-metrics-end {
  display: grid;
  grid-template-columns: 140px 120px;
  align-items: center;
  gap: 18px;
}

.task-card-metric--eta,
.task-card-metric--speed {
  display: grid;
  justify-content: end;
  gap: 6px;
  text-align: right;
}

.task-card-metric--eta {
  grid-template-columns: max-content 70px;
}

.task-card-metric--speed {
  grid-template-columns: max-content 76px;
}

.task-card-metric-label {
  color: var(--app-text-muted);
}

.task-card-metric-value {
  overflow: hidden;
  text-overflow: ellipsis;
  font-variant-numeric: tabular-nums;
}

.task-card-size {
  color: var(--app-text-secondary);
  text-align: left;
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
    grid-template-columns: minmax(130px, 1fr) max-content;
    gap: 8px;
  }

  .task-card-metrics-end {
    grid-template-columns: 132px 112px;
    gap: 14px;
  }

  .task-card-metric--eta {
    grid-template-columns: max-content 64px;
  }

  .task-card-metric--speed {
    grid-template-columns: max-content 70px;
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
