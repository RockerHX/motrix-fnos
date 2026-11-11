<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useMessage } from "naive-ui";
import TaskActions from "./TaskActions.vue";
import TaskFileConfirmDialog from "./TaskFileConfirmDialog.vue";
import { useTaskStore } from "../stores/taskStore";
import { useTaskStatusActions } from "../composables/useTaskStatusActions";
import { formatDateTime, language, useI18n } from "../../../i18n";
import { getErrorMessage } from "../../../app/utils/errors";
import { fnosHost, type FnosHostKind } from "../../../services/fnos";
import { getTaskFileContext } from "../services/taskService";
import { formatTaskError, formatTaskSizePair, formatTaskStatusLabel, getTaskSourceType } from "../utils/taskFormat";
import type { DownloadTask } from "../../../types/tasks";
import type {
  TaskActionConfirmTexts,
  TaskActionDetails,
  TaskActionDetailItem,
  TaskActionLabels,
  TaskActionPermissions,
  TaskActionState,
  TaskFileActionView,
} from "./taskActionViewModel";
import type { TaskFileAvailability, TaskFileContextResponse } from "../../../types/tasks";

const props = defineProps<{
  task: DownloadTask;
  mode: "desktop" | "mobile";
}>();

const taskStore = useTaskStore();
const message = useMessage();
const { t } = useI18n();
const showFileConfirm = ref(false);
const hostKind = ref<FnosHostKind>("unavailable");
const fileContext = ref<TaskFileContextResponse | null>(null);
const isFileContextLoading = ref(false);
let fileContextRequest = 0;
let pendingFileContext: Promise<TaskFileContextResponse | null> | null = null;
const { pauseTask, resumeTask } = useTaskStatusActions({ taskStore, message, t });

const hostSupported = computed(() => hostKind.value === "hosted" || hostKind.value === "mobile");
const fileActions = computed<TaskFileActionView>(() => ({
  hostSupported: hostSupported.value,
  loading: isFileContextLoading.value,
  context: fileContext.value,
}));
const isBtTask = computed(() => getTaskSourceType(props.task) !== "url");

onMounted(async () => {
  hostKind.value = await fnosHost.getHostKind();
});

watch(
  () => [props.task.id, props.task.status, props.task.updatedAt, language.value, taskStore.isRefreshing],
  () => {
    fileContextRequest += 1;
    pendingFileContext = null;
    fileContext.value = null;
    isFileContextLoading.value = false;
    if (props.task.status === "complete" && !taskStore.isRefreshing) void loadFileContext();
  },
  { immediate: true },
);

const localFileStatus = computed(() => {
  if (props.task.status !== "complete") return "";
  if (isFileContextLoading.value) return t("task.localFiles.checking");
  const availability = fileContext.value?.actions.availability;
  return availability ? availabilityMessage(availability) : t("task.localFiles.unknown");
});

const actionState = computed<TaskActionState>(() => ({
  isOperating: taskStore.isTaskOperating(props.task.id),
  isActionDisabled: taskStore.isTaskOperating(props.task.id) || taskStore.isRuntimeExiting || taskStore.isDownloadBlocked || props.task.readOnly === true,
  isRuntimeExiting: taskStore.isRuntimeExiting,
}));
const permissions = computed<TaskActionPermissions>(() => ({
  canPause: !props.task.readOnly && (props.task.status === "active" || props.task.status === "pending"),
  canResume: !props.task.readOnly && !props.task.confirmationRequired && (props.task.status === "paused" || props.task.status === "error"),
  canConfirmFiles: !props.task.readOnly && props.task.confirmationRequired && props.task.files.length > 0,
  canRedownload: !props.task.readOnly && props.task.status === "complete",
  canDelete: !props.task.readOnly && props.task.status !== "removed",
  canRestore: !props.task.readOnly && props.task.status === "removed" && (!props.task.cleanupState || props.task.cleanupState === "none"),
  canPermanentDelete: !props.task.readOnly && props.task.status === "removed" && (!props.task.cleanupState || props.task.cleanupState === "none"),
}));
const labels = computed<TaskActionLabels>(() => ({
  details: t("task.actions.details"),
  pause: t("task.actions.pause"),
  resume: t("task.actions.resume"),
  confirmFiles: t("task.actions.confirmFiles"),
  redownload: t("task.actions.redownload"),
  delete: t("task.actions.delete"),
  restore: t("task.actions.restore"),
  permanentDelete: t("task.actions.permanentDelete"),
  cancel: t("common.cancel"),
  close: t("common.close"),
  openFileManager: t("task.actions.openFileManager"),
  openFile: t("task.actions.openFile"),
  fileDetails: t("task.actions.fileDetails"),
  hostOnly: t("task.fileOperations.hostOnly"),
  technicalInfo: t("task.fileOperations.technicalInfo"),
  copyPath: t("common.copy"),
  copied: t("common.copied"),
  copyFailed: t("task.fileOperations.copyFailed"),
}));

const details = computed<TaskActionDetails>(() => {
  const semanticSaveDir = fileContext.value?.saveDir.displayPath || props.task.saveDir;
  const semanticFilePath = fileContext.value?.filePath?.displayPath || props.task.filePath || t("common.notAvailable");
  const items: TaskActionDetailItem[] = [
    { section: "download", label: t("task.detail.url"), value: props.task.url },
    { section: "download", label: t("task.detail.status"), value: formatTaskStatusLabel(props.task) },
    { section: "download", label: t("task.detail.size"), value: formatTaskSizePair(props.task) },
    { section: "download", label: t("task.detail.createdAt"), value: formatTimestamp(props.task.createdAt) },
    { section: "download", label: t("task.detail.updatedAt"), value: formatTimestamp(props.task.updatedAt) },
    { section: "file", label: t("task.detail.fileName"), value: props.task.fileName },
    { section: "file", label: t("task.detail.saveDir"), value: semanticSaveDir },
    { section: "file", label: t("task.detail.filePath"), value: semanticFilePath },
  ];
  if (localFileStatus.value) {
    items.push({ section: "file", label: t("task.localFiles.title"), value: localFileStatus.value });
  }

  if (props.task.errorMessage) {
    items.push({ section: "download", label: t("task.detail.errorReason"), value: formatTaskError(props.task) });
  }

  return {
    title: t("task.detail.title"),
    items,
    technicalItems: [
      { label: t("task.detail.gid"), value: props.task.gid || t("common.notAvailable") },
      { label: t("task.detail.saveDir"), value: props.task.saveDir },
      { label: t("task.detail.filePath"), value: props.task.filePath || t("common.notAvailable") },
    ],
  };
});
const confirmTexts = computed<TaskActionConfirmTexts>(() => ({
  redownloadTitle: t("task.redownload.title"),
  redownloadConfirmText: t(
    fileContext.value?.actions.availability === "partial_missing" ? "task.redownload.partialMissingConfirm"
      : fileContext.value?.actions.availability === "path_missing" || props.task.filesDeleted ? "task.redownload.missingConfirm"
      : "task.redownload.confirm",
    { name: props.task.fileName },
  ),
  restoreTitle: t("task.restore.title"),
  restoreConfirmText: t(props.task.filesDeleted ? "task.restore.restartConfirm" : "task.restore.confirm", { name: props.task.fileName }),
  deleteTitle: t("task.delete.title"),
  deleteConfirmText: t("task.delete.confirm", { name: props.task.fileName }),
  deleteFilesLabel: t("task.delete.files"),
  permanentDeleteTitle: t("task.permanentDelete.title"),
  permanentDeleteConfirmText: t("task.permanentDelete.confirm", { name: props.task.fileName }),
}));

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

async function updateTaskProxy(enabled: boolean) {
  if (isBtTask.value || !ensureCanOperate()) return;
  try {
    await taskStore.updateTaskProxy(props.task.id, enabled);
    message.success(t(enabled ? "task.proxy.enabled" : "task.proxy.disabled"));
  } catch (error) {
    message.error(getErrorMessage(error, t("task.operationFailed")));
  }
}

async function confirmRedownloadTask(useProxy: boolean) {
  if (!ensureCanOperate()) return;
  try {
    await taskStore.redownloadTask(props.task.id, isBtTask.value ? undefined : useProxy);
    message.success(t("task.actions.redownloaded"));
  } catch (error) {
    message.error(getErrorMessage(error, t("task.operationFailed")));
  }
}

async function confirmDeleteTask() {
  if (!ensureCanOperate()) return;
  try {
    await taskStore.deleteTask(props.task.id);
    message.success(t("task.actions.deleted"));
  } catch (error) {
    message.error(getErrorMessage(error, t("task.operationFailed")));
  }
}

async function confirmPermanentDeleteTask(deleteFiles: boolean) {
  if (!ensureCanOperate()) return;
  try {
    const pending = await taskStore.permanentlyDeleteTask(props.task.id, deleteFiles);
    message.success(t(pending ? "task.actions.cleanupSubmitted" : "task.actions.permanentlyDeleted"));
  } catch (error) {
    message.error(getErrorMessage(error, t("task.operationFailed")));
  }
}

async function restoreTask(useProxy: boolean) {
  if (!ensureCanOperate()) return;
  try {
    await taskStore.restoreTask(props.task.id, isBtTask.value ? undefined : useProxy, props.task.filesDeleted === true);
    message.success(t("task.actions.restored"));
  } catch (error) {
    message.error(getErrorMessage(error, t("task.operationFailed")));
  }
}

function loadFileContext(): Promise<TaskFileContextResponse | null> {
  if (pendingFileContext) return pendingFileContext;
  const request = ++fileContextRequest;
  isFileContextLoading.value = true;
  pendingFileContext = getTaskFileContext(props.task.id, language.value)
    .then((context) => {
      if (request !== fileContextRequest) return null;
      fileContext.value = context;
      return context;
    })
    .catch(() => {
      if (request === fileContextRequest) fileContext.value = null;
      return null;
    })
    .finally(() => {
      if (request !== fileContextRequest) return;
      isFileContextLoading.value = false;
      pendingFileContext = null;
    });
  return pendingFileContext;
}

async function prepareRedownload() {
  if (!ensureCanOperate()) return false;
  const context = await loadFileContext();
  if (!context) {
    message.error(t("task.fileOperations.contextFailed"));
    return false;
  }
  if (!["available", "path_missing", "partial_missing", "files_deleted"].includes(context.actions.availability)) {
    message.warning(availabilityMessage(context.actions.availability));
    return false;
  }
  return props.task.status === "complete" && ensureCanOperate();
}

function availabilityMessage(availability: TaskFileAvailability) {
  const key = `task.fileOperations.availability.${availability}` as const;
  return t(key);
}

async function openFileManager() {
  await runFileAction("fileManager");
}

async function openFile() {
  await runFileAction("file");
}

async function showFileDetails() {
  await runFileAction("details");
}

async function runFileAction(kind: "fileManager" | "file" | "details") {
  if (!ensureCanOperate()) return;
  if (!hostSupported.value) {
    message.warning(t("task.fileOperations.hostOnly"));
    return;
  }

  const context = await loadFileContext();
  if (!context) {
    message.error(t("task.fileOperations.contextFailed"));
    return;
  }
  if (!["available", "partial_missing"].includes(context.actions.availability)) {
    if (context) message.warning(availabilityMessage(context.actions.availability));
    return;
  }

  let result;
  if (kind === "fileManager" && context.actions.fileManagerPath) {
    result = await fnosHost.openFileManager(context.actions.fileManagerPath);
  } else if (kind === "file" && context.actions.openFilePath) {
    result = await fnosHost.openFile(context.actions.openFilePath);
  } else if (kind === "details" && context.actions.detailPaths.length > 0) {
    result = await fnosHost.showFileDetails(context.actions.detailPaths);
  } else {
    message.warning(t("task.fileOperations.unavailable"));
    return;
  }

  if (result.status === "failed") {
    message.error(t("task.fileOperations.failed"));
  } else if (result.status === "unsupported") {
    message.warning(t("task.fileOperations.hostOnly"));
  }
}

function ensureCanOperate() {
  if (taskStore.isDownloadBlocked || props.task.readOnly) {
    message.warning(t("task.legacyReadOnly"));
    return false;
  }
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
  <div class="task-actions-container">
    <p v-if="localFileStatus" class="task-local-file-status" role="status">{{ localFileStatus }}</p>
    <TaskActions
      :task="props.task"
      :mode="props.mode"
      :state="actionState"
      :permissions="permissions"
      :labels="labels"
      :details="details"
      :confirm-texts="confirmTexts"
      :file-actions="fileActions"
      :prepare-redownload="prepareRedownload"
      @pause="pauseTask(props.task)"
      @resume="resumeTask(props.task)"
      @confirm-files="showFileConfirm = true"
      @confirm-redownload="confirmRedownloadTask"
      @confirm-delete="confirmDeleteTask"
      @restore="restoreTask"
      @update-proxy="updateTaskProxy"
      @confirm-permanent-delete="confirmPermanentDeleteTask"
      @details-opened="loadFileContext"
      @open-file-manager="openFileManager"
      @open-file="openFile"
      @show-file-details="showFileDetails"
    />
    <TaskFileConfirmDialog
      v-model:show="showFileConfirm"
      :task="props.task"
      :is-loading="taskStore.isTaskOperating(props.task.id)"
      @confirm="confirmTaskFiles"
    />
  </div>
</template>

<style scoped src="./TaskActionsContainer.css"></style>
