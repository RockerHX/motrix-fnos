<script setup lang="ts">
import TaskActionsContainer from "./TaskActionsContainer.vue";
import TaskCardHeader from "./TaskCardHeader.vue";
import TaskErrorMessage from "./TaskErrorMessage.vue";
import TaskMetaItems from "./TaskMetaItems.vue";
import TaskProgressCell from "./TaskProgressCell.vue";
import { useMessage } from "naive-ui";
import { useTaskStore } from "../stores/taskStore";
import { useTaskStatusActions } from "../composables/useTaskStatusActions";
import { useI18n } from "../../../i18n";
import type { DownloadTask } from "../../../types/tasks";

const props = defineProps<{
  tasks: DownloadTask[];
}>();

const taskStore = useTaskStore();
const message = useMessage();
const { t } = useI18n();
const { handleTaskDoubleClick } = useTaskStatusActions({ taskStore, message, t });
</script>

<template>
  <section class="task-mobile-list">
    <article
      v-for="task in props.tasks"
      :key="task.id"
      class="task-card"
      @dblclick="handleTaskDoubleClick(task, $event)"
    >
      <TaskCardHeader :task="task" variant="mobile" />

      <p class="task-card-url" :title="task.url">{{ task.url }}</p>
      <TaskErrorMessage :task="task" variant="multi-line" />

      <div class="task-card-progress">
        <TaskProgressCell :task="task" />
      </div>

      <TaskMetaItems :task="task" variant="grid" />

      <footer class="task-card-actions">
        <TaskActionsContainer :task="task" compact />
      </footer>
    </article>
  </section>
</template>

<style scoped src="./TaskMobileList.css"></style>
