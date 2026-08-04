<script setup lang="ts">
import AppIcon from "../../../components/AppIcon.vue";
import { t } from "../../../i18n";
import TaskStatusBadge from "./TaskStatusBadge.vue";
import TaskSourceIcon from "./TaskSourceIcon.vue";
import type { DownloadTask } from "../../../types/tasks";

const props = withDefaults(
  defineProps<{
    task: DownloadTask;
    variant?: "desktop" | "mobile";
  }>(),
  {
    variant: "desktop",
  },
);
</script>

<template>
  <header class="task-card-header" :class="`task-card-header--${props.variant}`">
    <div class="task-card-title-group">
      <TaskSourceIcon :source-type="props.task.sourceType" :url="props.task.url" />
      <strong class="task-card-title" :title="props.task.fileName">{{ props.task.fileName }}</strong>
      <TaskStatusBadge :task="props.task" />
      <span
        v-if="props.task.useProxy"
        class="task-proxy-indicator"
        role="img"
        :title="t('task.proxy.iconHint')"
        :aria-label="t('task.proxy.iconHint')"
      >
        <AppIcon name="proxy" :size="14" />
      </span>
    </div>
    <aside v-if="$slots.actions" class="task-card-actions">
      <slot name="actions" />
    </aside>
  </header>
</template>

<style scoped src="./TaskCardHeader.css"></style>
