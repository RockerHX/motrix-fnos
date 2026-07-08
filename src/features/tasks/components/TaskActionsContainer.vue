<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useMessage } from "naive-ui";
import TaskActions from "./TaskActions.vue";
import TaskFileConfirmDialog from "./TaskFileConfirmDialog.vue";
import { useTaskStore } from "../stores/taskStore";
import { formatDateTime, useI18n } from "../../../i18n";
import { getErrorMessage } from "../../../app/utils/errors";
import { formatTaskError, formatTaskProgress, formatTaskSize, formatTaskSizePair, formatTaskStatusLabel } from "../utils/taskFormat";
import type { DownloadTask } from "../../../types/tasks";
import type {
  TaskActionConfirmTexts,
  TaskActionDetails,
  TaskActionLabels,
  TaskActionPermissions,
  TaskActionState,
} from "./taskActionViewModel";

const props = defineProps<{
  task: DownloadTask;
  compact?: boolean;
}>();

const taskStore = useTaskStore();
const message = useMessage();
const { t } = useI18n();
const showFileConfirm = ref(false);

const actionState = computed<TaskActionState>(() => ({
  isOperating: taskStore.isTaskOperating(props.task.id),
  isActionDisabled: taskStore.isTaskOperating(props.task.id) || taskStore.isRuntimeExiting,
  isRuntimeExiting: taskStore.isRuntimeExiting,
}));
const permissions = computed<TaskActionPermissions>(() => ({
  canPause: props.task.status === "active" || props.task.status === "pending",
  canResume: !props.task.confirmationRequired && (props.task.status === "paused" || props.task.status === "error"),
  canConfirmFiles: props.task.confirmationRequired && props.task.files.length > 0,
  canRedownload: props.task.status === "complete",
  canDelete: props.task.status !== "removed",
  canPermanentDelete: props.task.status === "removed",
}));
const labels = computed<TaskActionLabels>(() => ({
  details: t("task.actions.details"),
  pause: t("task.actions.pause"),
  resume: t("task.actions.resume"),
  confirmFiles: t("task.actions.confirmFiles"),
  redownload: t("task.actions.redownload"),
  delete: t("task.actions.delete"),
  permanentDelete: t("task.actions.permanentDelete"),
  cancel: t("common.cancel"),
  close: t("common.close"),
}));

watch(
  () => [props.task.id, props.task.confirmationRequired, props.task.files.length],
  () => {
    if (props.task.confirmationRequired && props.task.files.length > 0) {
      showFileConfirm.value = true;
    }
  },
  { immediate: true },
);
const details = computed<TaskActionDetails>(() => {
  const items = [
    { label: t("task.detail.fileName"), value: props.task.fileName },
    { label: t("task.detail.status"), value: formatTaskStatusLabel(props.task.status) },
    { label: t("task.detail.progress"), value: formatTaskProgress(props.task) },
    { label: t("task.detail.size"), value: formatTaskSizePair(props.task) },
    { label: t("task.detail.speed"), value: `${formatTaskSize(props.task.downloadSpeed)}/s` },
    { label: t("task.detail.saveDir"), value: props.task.saveDir },
    { label: t("task.detail.filePath"), value: props.task.filePath || t("common.notAvailable") },
    { label: t("task.detail.gid"), value: props.task.gid || t("common.notAvailable") },
    { label: t("task.detail.url"), value: props.task.url },
    { label: t("task.detail.createdAt"), value: formatTimestamp(props.task.createdAt) },
    { label: t("task.detail.updatedAt"), value: formatTimestamp(props.task.updatedAt) },
  ];

  if (props.task.errorMessage) {
    items.push({ label: t("task.detail.errorReason"), value: formatTaskError(props.task) });
  }

  return {
    title: t("task.detail.title"),
    items,
  };
});
const confirmTexts = computed<TaskActionConfirmTexts>(() => ({
  redownloadTitle: t("task.redownload.title"),
  redownloadConfirmText: t("task.redownload.confirm", { name: props.task.fileName }),
  deleteTitle: t("task.delete.title"),
  deleteConfirmText: t("task.delete.confirm", { name: props.task.fileName }),
  deleteFilesLabel: t("task.delete.files"),
  permanentDeleteTitle: t("task.permanentDelete.title"),
  permanentDeleteConfirmText: t("task.permanentDelete.confirm", { name: props.task.fileName }),
}));

async function pauseTask() {
  if (!ensureCanOperate()) return;
  try {
    await taskStore.pauseTask(props.task.id);
    message.success(t("task.actions.paused"));
  } catch (error) {
    message.error(getErrorMessage(error, t("task.operationFailed")));
  }
}

async function resumeTask() {
  if (!ensureCanOperate()) return;
  try {
    await taskStore.resumeTask(props.task.id);
    message.success(t("task.actions.resumed"));
  } catch (error) {
    message.error(getErrorMessage(error, t("task.operationFailed")));
  }
}

async function confirmTaskFiles(selectedFileIndexes: number[]) {
  if (!ensureCanOperate()) return;
  try {
    await taskStore.confirmTaskFiles(props.task.id, selectedFileIndexes);
    showFileConfirm.value = false;
    message.success(t("task.fileConfirm.started"));
  } catch (error) {
    message.error(getErrorMessage(error, t("task.operationFailed")));
  }
}

async function confirmRedownloadTask() {
  if (!ensureCanOperate()) return;
  try {
    await taskStore.redownloadTask(props.task.id);
    message.success(t("task.actions.redownloaded"));
  } catch (error) {
    message.error(getErrorMessage(error, t("task.operationFailed")));
  }
}

async function confirmDeleteTask(deleteFiles: boolean) {
  if (!ensureCanOperate()) return;
  try {
    await taskStore.deleteTask(props.task.id, deleteFiles);
    message.success(deleteFiles ? t("task.actions.deletedWithFiles") : t("task.actions.deleted"));
  } catch (error) {
    message.error(getErrorMessage(error, t("task.operationFailed")));
  }
}

async function confirmPermanentDeleteTask() {
  if (!ensureCanOperate()) return;
  try {
    await taskStore.permanentlyDeleteTask(props.task.id);
    message.success(t("task.actions.permanentlyDeleted"));
  } catch (error) {
    message.error(getErrorMessage(error, t("task.operationFailed")));
  }
}

function ensureCanOperate() {
  if (taskStore.isRuntimeExiting) {
    message.warning(t("task.runtimeExiting"));
    return false;
  }
  return true;
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
    :state="actionState"
    :permissions="permissions"
    :labels="labels"
    :details="details"
    :confirm-texts="confirmTexts"
    @pause="pauseTask"
    @resume="resumeTask"
    @confirm-files="showFileConfirm = true"
    @confirm-redownload="confirmRedownloadTask"
    @confirm-delete="confirmDeleteTask"
    @confirm-permanent-delete="confirmPermanentDeleteTask"
  />
  <TaskFileConfirmDialog
    v-model:show="showFileConfirm"
    :task="props.task"
    :is-loading="taskStore.isTaskOperating(props.task.id)"
    @confirm="confirmTaskFiles"
  />
</template>
