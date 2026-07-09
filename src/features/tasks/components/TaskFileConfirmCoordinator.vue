<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { storeToRefs } from "pinia";
import { useMessage } from "naive-ui";
import TaskFileConfirmDialog from "./TaskFileConfirmDialog.vue";
import { useTaskStore } from "../stores/taskStore";
import { useI18n } from "../../../i18n";
import { getErrorMessage } from "../../../app/utils/errors";

const taskStore = useTaskStore();
const { tasks } = storeToRefs(taskStore);
const message = useMessage();
const { t } = useI18n();
const dismissedTaskIds = ref<number[]>([]);

const confirmationTask = computed(
  () =>
    tasks.value.find(
      (task) =>
        task.confirmationRequired &&
        task.files.length > 0 &&
        !dismissedTaskIds.value.includes(task.id),
    ) ?? null,
);
const showFileConfirm = computed(() => confirmationTask.value !== null);

watch(
  () => tasks.value.map((task) => `${task.id}:${task.confirmationRequired}:${task.files.length}`).join("|"),
  () => {
    dismissedTaskIds.value = dismissedTaskIds.value.filter((taskId) =>
      tasks.value.some((task) => task.id === taskId && task.confirmationRequired && task.files.length > 0),
    );
  },
);

function handleShowUpdate(show: boolean) {
  if (show || !confirmationTask.value) {
    return;
  }
  dismissedTaskIds.value = [...new Set([...dismissedTaskIds.value, confirmationTask.value.id])];
}

async function confirmTaskFiles(selectedFileIndexes: number[]) {
  const task = confirmationTask.value;
  if (!task) {
    return;
  }

  try {
    await taskStore.confirmTaskFiles(task.id, selectedFileIndexes);
    dismissedTaskIds.value = dismissedTaskIds.value.filter((taskId) => taskId !== task.id);
    message.success(t("task.fileConfirm.started"));
  } catch (error) {
    message.error(getErrorMessage(error, t("task.operationFailed")));
  }
}
</script>

<template>
  <TaskFileConfirmDialog
    :show="showFileConfirm"
    :task="confirmationTask"
    :is-loading="confirmationTask ? taskStore.isTaskOperating(confirmationTask.id) : false"
    @update:show="handleShowUpdate"
    @confirm="confirmTaskFiles"
  />
</template>
