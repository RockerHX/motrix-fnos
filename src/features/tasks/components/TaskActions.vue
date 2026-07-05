<script setup lang="ts">
import { computed, ref, watch } from "vue";
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
import { formatDateTime, useI18n, type TranslationKey } from "../../../i18n";
import type { DownloadTask, DownloadTaskStatus } from "../../../types/tasks";

const props = defineProps<{
  task: DownloadTask;
  compact?: boolean;
}>();

const taskStore = useTaskStore();
const message = useMessage();
const { t } = useI18n();
const showDeleteConfirm = ref(false);
const showPermanentDeleteConfirm = ref(false);
const showRedownloadConfirm = ref(false);
const showDetails = ref(false);
const deleteFiles = ref(false);

const isOperating = computed(() => taskStore.isTaskOperating(props.task.id));
const isActionDisabled = computed(() => isOperating.value || taskStore.isRuntimeExiting);
const canPause = computed(() => props.task.status === "active" || props.task.status === "pending");
const canResume = computed(() => props.task.status === "paused" || props.task.status === "error");
const canRedownload = computed(() => props.task.status === "complete");
const canDelete = computed(() => props.task.status !== "removed");
const canPermanentDelete = computed(() => props.task.status === "removed");
const progressText = computed(() => {
  if (props.task.totalLength <= 0) {
    return "0.00%";
  }

  const percentage = Math.min(100, (props.task.completedLength / props.task.totalLength) * 100);
  return `${percentage.toFixed(2)}%`;
});

watch(
  () => taskStore.isRuntimeExiting,
  (isRuntimeExiting) => {
    if (!isRuntimeExiting) {
      return;
    }

    showDeleteConfirm.value = false;
    showPermanentDeleteConfirm.value = false;
    showRedownloadConfirm.value = false;
    showDetails.value = false;
    deleteFiles.value = false;
  },
);

function ensureCanOperate() {
  if (taskStore.isRuntimeExiting) {
    message.warning(t("task.runtimeExiting"));
    return false;
  }
  return true;
}

async function pauseTask() {
  if (!ensureCanOperate()) return;
  try {
    await taskStore.pauseTask(props.task.id);
    message.success(t("task.actions.paused"));
  } catch (error) {
    message.error(getErrorMessage(error));
  }
}

async function resumeTask() {
  if (!ensureCanOperate()) return;
  try {
    await taskStore.resumeTask(props.task.id);
    message.success(t("task.actions.resumed"));
  } catch (error) {
    message.error(getErrorMessage(error));
  }
}

async function confirmRedownloadTask() {
  if (!ensureCanOperate()) return;
  try {
    await taskStore.redownloadTask(props.task.id);
    showRedownloadConfirm.value = false;
    message.success(t("task.actions.redownloaded"));
  } catch (error) {
    message.error(getErrorMessage(error));
  }
}

function openDeleteConfirm() {
  if (!ensureCanOperate()) return;
  deleteFiles.value = false;
  showDeleteConfirm.value = true;
}

async function confirmDeleteTask() {
  if (!ensureCanOperate()) return;
  try {
    await taskStore.deleteTask(props.task.id, deleteFiles.value);
    showDeleteConfirm.value = false;
    message.success(deleteFiles.value ? t("task.actions.deletedWithFiles") : t("task.actions.deleted"));
  } catch (error) {
    message.error(getErrorMessage(error));
  }
}

function openPermanentDeleteConfirm() {
  if (!ensureCanOperate()) return;
  showPermanentDeleteConfirm.value = true;
}

async function confirmPermanentDeleteTask() {
  if (!ensureCanOperate()) return;
  try {
    await taskStore.permanentlyDeleteTask(props.task.id);
    showPermanentDeleteConfirm.value = false;
    message.success(t("task.actions.permanentlyDeleted"));
  } catch (error) {
    message.error(getErrorMessage(error));
  }
}

function getErrorMessage(error: unknown) {
  if (error instanceof Error) {
    return error.message;
  }

  const message = String(error);
  return message || t("task.operationFailed");
}

function statusLabel(status: DownloadTaskStatus) {
  const labels: Record<DownloadTaskStatus, TranslationKey> = {
    pending: "task.status.pending",
    active: "task.status.active",
    paused: "task.status.paused",
    complete: "task.status.complete",
    error: "task.status.error",
    removed: "task.status.removed",
  };
  return t(labels[status]);
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

  return formatDateTime(timestamp);
}
</script>

<template>
  <div v-if="props.compact" class="compact-actions">
    <NButton size="small" secondary :disabled="isActionDisabled" @click="showDetails = true">{{ t("task.actions.details") }}</NButton>
    <NButton v-if="canPause" size="small" secondary :loading="isOperating" :disabled="isActionDisabled" @click="pauseTask">
      {{ t("task.actions.pause") }}
    </NButton>
    <NButton v-if="canResume" size="small" secondary :loading="isOperating" :disabled="isActionDisabled" @click="resumeTask">
      {{ t("task.actions.resume") }}
    </NButton>
    <NButton v-if="canRedownload" size="small" secondary :disabled="isActionDisabled" @click="showRedownloadConfirm = true">
      {{ t("task.actions.redownload") }}
    </NButton>
    <NButton v-if="canDelete" size="small" secondary type="error" :disabled="isActionDisabled" @click="openDeleteConfirm">
      {{ t("task.actions.delete") }}
    </NButton>
    <NButton
      v-if="canPermanentDelete"
      size="small"
      secondary
      type="error"
      :loading="isOperating"
      :disabled="isActionDisabled"
      @click="openPermanentDeleteConfirm"
    >
      {{ t("task.actions.permanentDelete") }}
    </NButton>
  </div>
  <NSpace v-else :size="6" wrap>
    <NButton size="small" secondary :disabled="isActionDisabled" @click="showDetails = true">{{ t("task.actions.details") }}</NButton>
    <NButton v-if="canPause" size="small" secondary :loading="isOperating" :disabled="isActionDisabled" @click="pauseTask">
      {{ t("task.actions.pause") }}
    </NButton>
    <NButton v-if="canResume" size="small" secondary :loading="isOperating" :disabled="isActionDisabled" @click="resumeTask">
      {{ t("task.actions.resume") }}
    </NButton>
    <NButton v-if="canRedownload" size="small" secondary :disabled="isActionDisabled" @click="showRedownloadConfirm = true">
      {{ t("task.actions.redownload") }}
    </NButton>
    <NButton v-if="canDelete" size="small" secondary type="error" :disabled="isActionDisabled" @click="openDeleteConfirm">
      {{ t("task.actions.delete") }}
    </NButton>
    <NButton
      v-if="canPermanentDelete"
      size="small"
      secondary
      type="error"
      :loading="isOperating"
      :disabled="isActionDisabled"
      @click="openPermanentDeleteConfirm"
    >
      {{ t("task.actions.permanentDelete") }}
    </NButton>
  </NSpace>

  <NModal v-model:show="showDetails">
    <NCard class="task-detail-card" role="dialog" aria-modal="true" :title="t('task.detail.title')">
      <NDescriptions :column="1" label-placement="left" bordered>
        <NDescriptionsItem :label="t('task.detail.fileName')">{{ task.fileName }}</NDescriptionsItem>
        <NDescriptionsItem :label="t('task.detail.status')">{{ statusLabel(task.status) }}</NDescriptionsItem>
        <NDescriptionsItem :label="t('task.detail.progress')">{{ progressText }}</NDescriptionsItem>
        <NDescriptionsItem :label="t('task.detail.size')">
          {{ formatSize(task.completedLength) }} / {{ task.totalLength > 0 ? formatSize(task.totalLength) : t("common.unknown") }}
        </NDescriptionsItem>
        <NDescriptionsItem :label="t('task.detail.speed')">{{ formatSize(task.downloadSpeed) }}/s</NDescriptionsItem>
        <NDescriptionsItem :label="t('task.detail.saveDir')">{{ task.saveDir }}</NDescriptionsItem>
        <NDescriptionsItem :label="t('task.detail.filePath')">{{ task.filePath || t("common.notAvailable") }}</NDescriptionsItem>
        <NDescriptionsItem :label="t('task.detail.gid')">{{ task.gid || t("common.notAvailable") }}</NDescriptionsItem>
        <NDescriptionsItem :label="t('task.detail.url')">{{ task.url }}</NDescriptionsItem>
        <NDescriptionsItem :label="t('task.detail.createdAt')">{{ formatTimestamp(task.createdAt) }}</NDescriptionsItem>
        <NDescriptionsItem :label="t('task.detail.updatedAt')">{{ formatTimestamp(task.updatedAt) }}</NDescriptionsItem>
        <NDescriptionsItem v-if="task.errorMessage" :label="t('task.detail.errorReason')">
          {{ task.errorCode ? t("task.errorCode", { code: task.errorCode }) : "" }}{{ task.errorMessage }}
        </NDescriptionsItem>
      </NDescriptions>

      <template #footer>
        <NSpace justify="end">
          <NButton @click="showDetails = false">{{ t("common.close") }}</NButton>
        </NSpace>
      </template>
    </NCard>
  </NModal>

  <NModal v-model:show="showRedownloadConfirm" :mask-closable="!isOperating">
    <NCard class="redownload-confirm-card" role="dialog" aria-modal="true" :title="t('task.redownload.title')">
      <p class="delete-confirm-text">
        {{ t("task.redownload.confirm", { name: task.fileName }) }}
      </p>

      <template #footer>
        <NSpace justify="end">
          <NButton :disabled="isActionDisabled" @click="showRedownloadConfirm = false">{{ t("common.cancel") }}</NButton>
          <NButton type="primary" :loading="isOperating" :disabled="isActionDisabled" @click="confirmRedownloadTask">
            {{ t("task.actions.redownload") }}
          </NButton>
        </NSpace>
      </template>
    </NCard>
  </NModal>

  <NModal v-model:show="showDeleteConfirm" :mask-closable="!isOperating">
    <NCard class="delete-confirm-card" role="dialog" aria-modal="true" :title="t('task.delete.title')">
      <p class="delete-confirm-text">{{ t("task.delete.confirm", { name: task.fileName }) }}</p>
      <NCheckbox v-model:checked="deleteFiles">{{ t("task.delete.files") }}</NCheckbox>

      <template #footer>
        <NSpace justify="end">
          <NButton :disabled="isActionDisabled" @click="showDeleteConfirm = false">{{ t("common.cancel") }}</NButton>
          <NButton type="error" :loading="isOperating" :disabled="isActionDisabled" @click="confirmDeleteTask">
            {{ t("task.actions.delete") }}
          </NButton>
        </NSpace>
      </template>
    </NCard>
  </NModal>

  <NModal v-model:show="showPermanentDeleteConfirm" :mask-closable="!isOperating">
    <NCard class="permanent-delete-confirm-card" role="dialog" aria-modal="true" :title="t('task.permanentDelete.title')">
      <p class="delete-confirm-text">
        {{ t("task.permanentDelete.confirm", { name: task.fileName }) }}
      </p>

      <template #footer>
        <NSpace justify="end">
          <NButton :disabled="isActionDisabled" @click="showPermanentDeleteConfirm = false">{{ t("common.cancel") }}</NButton>
          <NButton type="error" :loading="isOperating" :disabled="isActionDisabled" @click="confirmPermanentDeleteTask">
            {{ t("task.actions.permanentDelete") }}
          </NButton>
        </NSpace>
      </template>
    </NCard>
  </NModal>
</template>

<style scoped>
.compact-actions {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 8px;
}

.compact-actions :deep(.n-button) {
  width: 100%;
}

.delete-confirm-card,
.permanent-delete-confirm-card,
.redownload-confirm-card {
  width: min(420px, calc(100vw - 48px));
  max-height: calc(var(--app-viewport-height) - 48px);
  overflow: auto;
}

.task-detail-card {
  width: min(720px, calc(100vw - 48px));
  max-height: calc(var(--app-viewport-height) - 48px);
  overflow: auto;
}

.delete-confirm-text {
  margin: 0 0 14px;
  color: #d7dfd8;
}

:deep(.n-descriptions-table-content__content) {
  word-break: break-all;
}

@media (max-width: 767px) {
  .delete-confirm-card,
  .permanent-delete-confirm-card,
  .redownload-confirm-card,
  .task-detail-card {
    width: calc(100vw - 24px);
    max-height: calc(var(--app-viewport-height) - 24px);
  }
}
</style>
