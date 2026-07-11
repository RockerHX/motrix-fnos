<script setup lang="ts">
import { computed } from "vue";
import { formatTaskError } from "../utils/taskFormat";
import type { DownloadTask } from "../../../types/tasks";

const props = withDefaults(
  defineProps<{
    task: DownloadTask;
    variant?: "single-line" | "multi-line";
  }>(),
  {
    variant: "single-line",
  },
);

const errorText = computed(() => formatTaskError(props.task));
</script>

<template>
  <div class="task-card-error-slot" data-test="task-card-error-slot">
    <p
      v-if="props.task.status === 'error'"
      class="task-card-error"
      :class="`task-card-error--${props.variant}`"
      :title="errorText"
    >
      {{ errorText }}
    </p>
  </div>
</template>

<style scoped>
.task-card-error-slot {
  min-width: 0;
  min-height: 0;
}

.task-card-error {
  overflow: hidden;
  margin: 0;
  color: var(--app-text-danger);
  line-height: 1.35;
}

.task-card-error--single-line {
  font-size: 11px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.task-card-error--multi-line {
  display: -webkit-box;
  font-size: 13px;
  line-height: 1.5;
  word-break: break-word;
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 2;
}

@media (max-width: 767px) {
  .task-card-error--multi-line {
    font-size: 12px;
    line-height: 1.5;
  }
}
</style>
