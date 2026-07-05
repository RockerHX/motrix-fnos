<script setup lang="ts">
import TaskDesktopTable from "./TaskDesktopTable.vue";
import type { DownloadTask } from "../../../types/tasks";

defineProps<{
  tasks: DownloadTask[];
}>();
</script>

<template>
  <TaskDesktopTable :tasks="tasks" />
</template>
