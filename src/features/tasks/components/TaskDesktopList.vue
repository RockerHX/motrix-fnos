<script setup lang="ts">
import { NList, NListItem, useMessage } from "naive-ui";
import TaskDesktopCard from "./TaskDesktopCard.vue";
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
  <section class="task-desktop-list" data-test="task-desktop-list">
    <NList class="task-desktop-list-scroll" hoverable show-divider>
      <NListItem v-for="task in props.tasks" :key="task.id">
        <TaskDesktopCard :task="task" @dblclick="handleTaskDoubleClick(task, $event)" />
      </NListItem>
    </NList>
  </section>
</template>

<style scoped src="./TaskDesktopList.css"></style>
