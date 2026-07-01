<script setup lang="ts">
import { computed, ref } from "vue";
import {
  NButton,
  NCard,
  NCheckbox,
  NDescriptions,
  NDescriptionsItem,
  NModal,
  NSpace,
  useMessage,
} from "naive-ui";
import { useTaskStore } from "../stores/taskStore";
import type { DownloadTask } from "../../../types/tasks";

const props = defineProps<{
  task: DownloadTask;
}>();

const taskStore = useTaskStore();
const message = useMessage();
const showDeleteConfirm = ref(false);
const showRedownloadConfirm = ref(false);
const showDetails = ref(false);
const deleteFiles = ref(false);

const isOperating = computed(() => taskStore.isTaskOperating(props.task.id));
const canPause = computed(() => props.task.status === "active" || props.task.status === "pending");
const canResume = computed(() => props.task.status === "paused" || props.task.status === "error");
const canRedownload = computed(() => props.task.status === "complete");
const canDelete = computed(() => props.task.status !== "removed");
const progressText = computed(() => {
  if (props.task.totalLength <= 0) {
    return "0.00%";
  }

  const percentage = Math.min(100, (props.task.completedLength / props.task.totalLength) * 100);
  return `${percentage.toFixed(2)}%`;
});

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

async function confirmRedownloadTask() {
  try {
    await taskStore.redownloadTask(props.task.id);
    showRedownloadConfirm.value = false;
    message.success("任务已重新下载，原文件已移入回收站");
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

function formatSize(size: number) {
  if (size <= 0) {
    return "0 B";
  }

  const units = ["B", "KB", "MB", "GB", "TB"];
  let value = size;
  let unitIndex = 0;

  while (value >= 1024 && unitIndex < units.length - 1) {
    value /= 1024;
    unitIndex += 1;
  }

  return `${value.toFixed(value >= 10 || unitIndex === 0 ? 0 : 1)} ${units[unitIndex]}`;
}

function formatTimestamp(timestamp: number) {
  if (!timestamp) {
    return "--";
  }

  return new Date(timestamp).toLocaleString();
}
</script>

<template>
  <NSpace :size="6" wrap>
    <NButton size="small" secondary :disabled="isOperating" @click="showDetails = true">详情</NButton>
    <NButton v-if="canPause" size="small" secondary :loading="isOperating" @click="pauseTask">暂停</NButton>
    <NButton v-if="canResume" size="small" secondary :loading="isOperating" @click="resumeTask">继续</NButton>
    <NButton v-if="canRedownload" size="small" secondary :disabled="isOperating" @click="showRedownloadConfirm = true">
      重新下载
    </NButton>
    <NButton v-if="canDelete" size="small" secondary type="error" :disabled="isOperating" @click="openDeleteConfirm">
      删除
    </NButton>
  </NSpace>

  <NModal v-model:show="showDetails">
    <NCard class="task-detail-card" role="dialog" aria-modal="true" title="任务详情">
      <NDescriptions :column="1" label-placement="left" bordered>
        <NDescriptionsItem label="任务名称">{{ task.fileName }}</NDescriptionsItem>
        <NDescriptionsItem label="状态">{{ task.status }}</NDescriptionsItem>
        <NDescriptionsItem label="进度">{{ progressText }}</NDescriptionsItem>
        <NDescriptionsItem label="已下载 / 总大小">
          {{ formatSize(task.completedLength) }} / {{ task.totalLength > 0 ? formatSize(task.totalLength) : "未知" }}
        </NDescriptionsItem>
        <NDescriptionsItem label="速度">{{ formatSize(task.downloadSpeed) }}/s</NDescriptionsItem>
        <NDescriptionsItem label="保存路径">{{ task.saveDir }}</NDescriptionsItem>
        <NDescriptionsItem label="文件路径">{{ task.filePath || "--" }}</NDescriptionsItem>
        <NDescriptionsItem label="GID">{{ task.gid || "--" }}</NDescriptionsItem>
        <NDescriptionsItem label="下载链接">{{ task.url }}</NDescriptionsItem>
        <NDescriptionsItem label="创建时间">{{ formatTimestamp(task.createdAt) }}</NDescriptionsItem>
        <NDescriptionsItem label="更新时间">{{ formatTimestamp(task.updatedAt) }}</NDescriptionsItem>
        <NDescriptionsItem v-if="task.errorMessage" label="错误原因">
          {{ task.errorCode ? `错误码 ${task.errorCode}：` : "" }}{{ task.errorMessage }}
        </NDescriptionsItem>
      </NDescriptions>

      <template #footer>
        <NSpace justify="end">
          <NButton @click="showDetails = false">关闭</NButton>
        </NSpace>
      </template>
    </NCard>
  </NModal>

  <NModal v-model:show="showRedownloadConfirm" :mask-closable="!isOperating">
    <NCard class="redownload-confirm-card" role="dialog" aria-modal="true" title="重新下载任务">
      <p class="delete-confirm-text">
        重新下载会把“{{ task.fileName }}”当前本地文件移入回收站，然后从 0 开始下载。确定继续吗？
      </p>

      <template #footer>
        <NSpace justify="end">
          <NButton :disabled="isOperating" @click="showRedownloadConfirm = false">取消</NButton>
          <NButton type="primary" :loading="isOperating" @click="confirmRedownloadTask">重新下载</NButton>
        </NSpace>
      </template>
    </NCard>
  </NModal>

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
.delete-confirm-card,
.redownload-confirm-card {
  width: min(420px, calc(100vw - 48px));
}

.task-detail-card {
  width: min(720px, calc(100vw - 48px));
}

.delete-confirm-text {
  margin: 0 0 14px;
  color: #d7dfd8;
}

:deep(.n-descriptions-table-content__content) {
  word-break: break-all;
}
</style>
