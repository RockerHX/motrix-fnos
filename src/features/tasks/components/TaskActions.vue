<script setup lang="ts">
import { computed, ref } from "vue";
import { NButton, NCard, NCheckbox, NModal, NSpace, useMessage } from "naive-ui";
import { useTaskStore } from "../stores/taskStore";
import type { DownloadTask } from "../../../types/tasks";

const props = defineProps<{
  task: DownloadTask;
}>();

const taskStore = useTaskStore();
const message = useMessage();
const showDeleteConfirm = ref(false);
const deleteFiles = ref(false);

const isOperating = computed(() => taskStore.isTaskOperating(props.task.id));
const canPause = computed(() => props.task.status === "active" || props.task.status === "pending");
const canResume = computed(() => props.task.status === "paused" || props.task.status === "error");
const canDelete = computed(() => props.task.status !== "removed");

async function pauseTask() {
  try {
    await taskStore.pauseTask(props.task.id);
    message.success("任务已暂停");
  } catch (error) {
    message.error(getErrorMessage(error));
  }
}

async function resumeTask() {
  try {
    await taskStore.resumeTask(props.task.id);
    message.success("任务已继续");
  } catch (error) {
    message.error(getErrorMessage(error));
  }
}

function openDeleteConfirm() {
  deleteFiles.value = false;
  showDeleteConfirm.value = true;
}

async function confirmDeleteTask() {
  try {
    await taskStore.deleteTask(props.task.id, deleteFiles.value);
    showDeleteConfirm.value = false;
    message.success(deleteFiles.value ? "任务和本地文件已删除" : "任务已删除");
  } catch (error) {
    message.error(getErrorMessage(error));
  }
}

function getErrorMessage(error: unknown) {
  if (error instanceof Error) {
    return error.message;
  }

  const message = String(error);
  return message || "操作失败，请稍后重试";
}
</script>

<template>
  <NSpace :size="6" wrap>
    <NButton v-if="canPause" size="small" secondary :loading="isOperating" @click="pauseTask">暂停</NButton>
    <NButton v-if="canResume" size="small" secondary :loading="isOperating" @click="resumeTask">继续</NButton>
    <NButton v-if="canDelete" size="small" secondary type="error" :disabled="isOperating" @click="openDeleteConfirm">
      删除
    </NButton>
  </NSpace>

  <NModal v-model:show="showDeleteConfirm" :mask-closable="!isOperating">
    <NCard class="delete-confirm-card" role="dialog" aria-modal="true" title="删除下载任务">
      <p class="delete-confirm-text">确定要删除“{{ task.fileName }}”吗？</p>
      <NCheckbox v-model:checked="deleteFiles">同时删除本地文件</NCheckbox>

      <template #footer>
        <NSpace justify="end">
          <NButton :disabled="isOperating" @click="showDeleteConfirm = false">取消</NButton>
          <NButton type="error" :loading="isOperating" @click="confirmDeleteTask">删除</NButton>
        </NSpace>
      </template>
    </NCard>
  </NModal>
</template>

<style scoped>
.delete-confirm-card {
  width: min(420px, calc(100vw - 48px));
}

.delete-confirm-text {
  margin: 0 0 14px;
  color: #d7dfd8;
}
</style>
