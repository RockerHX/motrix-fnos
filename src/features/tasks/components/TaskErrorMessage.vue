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

<style scoped src="./TaskErrorMessage.css"></style>
