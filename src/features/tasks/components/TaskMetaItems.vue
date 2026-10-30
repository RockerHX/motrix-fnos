<script setup lang="ts">
import { computed } from "vue";
import { t } from "../../../i18n";
import { formatTaskEta, formatTaskSpeed, formatTaskSizePair } from "../utils/taskFormat";
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
const sizeLabel = computed(() => formatTaskSizePair(props.task));
const speedLabel = computed(() => formatTaskSpeed(props.task));
const etaLabel = computed(() => formatTaskEta(props.task));
</script>

<template>
  <footer v-if="props.variant === 'inline'" class="task-card-meta task-card-meta--inline">
    <span class="task-card-size task-card-metric" data-test="task-size-metric">
      <span class="task-card-metric-label">{{ t("task.table.size") }}</span>
      <span class="task-card-metric-value" :title="sizeLabel">{{ sizeLabel }}</span>
    </span>
    <span class="task-card-metrics-end" data-test="task-dynamic-metrics">
      <span
        class="task-card-metric task-card-metric--speed"
        data-test="task-speed-metric"
        :aria-label="`${t('task.table.speed')} ${speedLabel}`"
      >
        <span class="task-card-metric-label">{{ t("task.table.speed") }}</span>
        <span class="task-card-metric-value" :title="speedLabel">{{ speedLabel }}</span>
      </span>
      <span
        class="task-card-metric task-card-metric--eta"
        data-test="task-eta-metric"
        :aria-label="`${t('task.table.eta')} ${etaLabel}`"
      >
        <span class="task-card-metric-label">{{ t("task.table.eta") }}</span>
        <span class="task-card-metric-value" :title="etaLabel">{{ etaLabel }}</span>
      </span>
    </span>
  </footer>

  <dl v-else class="task-card-meta task-card-meta--grid">
    <div data-test="task-size-metric">
      <dt>{{ t("task.table.size") }}</dt>
      <dd :title="sizeLabel">{{ sizeLabel }}</dd>
    </div>
    <div data-test="task-speed-metric">
      <dt>{{ t("task.table.speed") }}</dt>
      <dd :title="speedLabel">{{ speedLabel }}</dd>
    </div>
    <div data-test="task-eta-metric">
      <dt>{{ t("task.table.eta") }}</dt>
      <dd :title="etaLabel">{{ etaLabel }}</dd>
    </div>
  </dl>
</template>

<style scoped src="./TaskMetaItems.css"></style>
