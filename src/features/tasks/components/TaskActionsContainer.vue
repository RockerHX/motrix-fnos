<script setup lang="ts">
import { computed } from "vue";
import { useMessage } from "naive-ui";
import TaskActions from "./TaskActions.vue";
import { useTaskStore } from "../stores/taskStore";
import { formatDateTime, useI18n } from "../../../i18n";
import { formatTaskError, formatTaskProgress, formatTaskSize, formatTaskSizePair, formatTaskStatusLabel } from "../utils/taskFormat";
import type { DownloadTask } from "../../../types/tasks";

const props = defineProps<{
  task: DownloadTask;
  compact?: boolean;
}>();

const taskStore = useTaskStore();
const message = useMessage();
const { t } = useI18n();

const isOperating = computed(() => taskStore.isTaskOperating(props.task.id));
const isActionDisabled = computed(() => isOperating.value || taskStore.isRuntimeExiting);
const canPause = computed(() => props.task.status === "active" || props.task.status === "pending");
const canResume = computed(() => props.task.status === "paused" || props.task.status === "error");
const canRedownload = computed(() => props.task.status === "complete");
const canDelete = computed(() => props.task.status !== "removed");
const canPermanentDelete = computed(() => props.task.status === "removed");
const progressText = computed(() => formatTaskProgress(props.task));
const detailSize = computed(() => formatTaskSizePair(props.task));
const detailSpeed = computed(() => `${formatTaskSize(props.task.downloadSpeed)}/s`);
const detailFilePath = computed(() => props.task.filePath || t("common.notAvailable"));
const detailGid = computed(() => props.task.gid || t("common.notAvailable"));
const detailCreatedAt = computed(() => formatTimestamp(props.task.createdAt));
const detailUpdatedAt = computed(() => formatTimestamp(props.task.updatedAt));
const detailErrorReason = computed(() => (props.task.errorMessage ? formatTaskError(props.task) : undefined));

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
    message.success(t("task.actions.redownloaded"));
  } catch (error) {
    message.error(getErrorMessage(error));
  }
}

async function confirmDeleteTask(deleteFiles: boolean) {
  if (!ensureCanOperate()) return;
  try {
    await taskStore.deleteTask(props.task.id, deleteFiles);
    message.success(deleteFiles ? t("task.actions.deletedWithFiles") : t("task.actions.deleted"));
  } catch (error) {
    message.error(getErrorMessage(error));
  }
}

async function confirmPermanentDeleteTask() {
  if (!ensureCanOperate()) return;
  try {
    await taskStore.permanentlyDeleteTask(props.task.id);
    message.success(t("task.actions.permanentlyDeleted"));
  } catch (error) {
    message.error(getErrorMessage(error));
  }
}

function ensureCanOperate() {
  if (taskStore.isRuntimeExiting) {
    message.warning(t("task.runtimeExiting"));
    return false;
  }
  return true;
}

function getErrorMessage(error: unknown) {
  if (error instanceof Error) {
    return error.message;
  }

  const message = String(error);
  return message || t("task.operationFailed");
}

function formatTimestamp(timestamp: number) {
  if (!timestamp) {
    return "--";
  }

  return formatDateTime(timestamp);
}
</script>

<template>
  <TaskActions
    :compact="props.compact"
    :is-operating="isOperating"
    :is-action-disabled="isActionDisabled"
    :is-runtime-exiting="taskStore.isRuntimeExiting"
    :can-pause="canPause"
    :can-resume="canResume"
    :can-redownload="canRedownload"
    :can-delete="canDelete"
    :can-permanent-delete="canPermanentDelete"
    :details-label="t('task.actions.details')"
    :pause-label="t('task.actions.pause')"
    :resume-label="t('task.actions.resume')"
    :redownload-label="t('task.actions.redownload')"
    :delete-label="t('task.actions.delete')"
    :permanent-delete-label="t('task.actions.permanentDelete')"
    :cancel-label="t('common.cancel')"
    :close-label="t('common.close')"
    :detail-title="t('task.detail.title')"
    :detail-file-name-label="t('task.detail.fileName')"
    :detail-status-label="t('task.detail.status')"
    :detail-progress-label="t('task.detail.progress')"
    :detail-size-label="t('task.detail.size')"
    :detail-speed-label="t('task.detail.speed')"
    :detail-save-dir-label="t('task.detail.saveDir')"
    :detail-file-path-label="t('task.detail.filePath')"
    :detail-gid-label="t('task.detail.gid')"
    :detail-url-label="t('task.detail.url')"
    :detail-created-at-label="t('task.detail.createdAt')"
    :detail-updated-at-label="t('task.detail.updatedAt')"
    :detail-error-reason-label="t('task.detail.errorReason')"
    :detail-file-name="props.task.fileName"
    :detail-status="formatTaskStatusLabel(props.task.status)"
    :detail-progress="progressText"
    :detail-size="detailSize"
    :detail-speed="detailSpeed"
    :detail-save-dir="props.task.saveDir"
    :detail-file-path="detailFilePath"
    :detail-gid="detailGid"
    :detail-url="props.task.url"
    :detail-created-at="detailCreatedAt"
    :detail-updated-at="detailUpdatedAt"
    :detail-error-reason="detailErrorReason"
    :redownload-title="t('task.redownload.title')"
    :redownload-confirm-text="t('task.redownload.confirm', { name: props.task.fileName })"
    :delete-title="t('task.delete.title')"
    :delete-confirm-text="t('task.delete.confirm', { name: props.task.fileName })"
    :delete-files-label="t('task.delete.files')"
    :permanent-delete-title="t('task.permanentDelete.title')"
    :permanent-delete-confirm-text="t('task.permanentDelete.confirm', { name: props.task.fileName })"
    @pause="pauseTask"
    @resume="resumeTask"
    @confirm-redownload="confirmRedownloadTask"
    @confirm-delete="confirmDeleteTask"
    @confirm-permanent-delete="confirmPermanentDeleteTask"
  />
</template>
