<script setup lang="ts">
import { useMobileLayout } from "../../../app/composables/useMobileLayout";
import TaskMobileList from "./TaskMobileList.vue";
import TaskDesktopList from "./TaskDesktopList.vue";
import type { DownloadTask } from "../../../types/tasks";

defineProps<{
  tasks: DownloadTask[];
}>();

const { isMobileLayout } = useMobileLayout();
</script>

<template>
  <TaskMobileList v-if="isMobileLayout" :tasks="tasks" />
  <TaskDesktopList v-else :tasks="tasks" />
</template>
