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

<style scoped src="./TaskMetaItems.css"></style>
