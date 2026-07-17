<script setup lang="ts">
import { NList, NListItem } from "naive-ui";
import TaskDesktopCard from "./TaskDesktopCard.vue";
import type { DownloadTask } from "../../../types/tasks";

const props = defineProps<{
  tasks: DownloadTask[];
}>();
</script>

<template>
  <section class="task-desktop-list" data-test="task-desktop-list">
    <NList class="task-desktop-list-scroll" hoverable show-divider>
      <NListItem v-for="task in props.tasks" :key="task.id">
        <TaskDesktopCard :task="task" />
      </NListItem>
    </NList>
  </section>
</template>

<style scoped src="./TaskDesktopList.css"></style>
