import { computed, ref } from "vue";
import { useTaskStore } from "../stores/taskStore";
import { runTaskToolbarBatch, useTaskToolbar } from "./useTaskToolbar";
import type { TranslationKey, TranslationParams } from "../../../i18n";
import type { DownloadTask } from "../../../types/tasks";

type TaskToolbar = ReturnType<typeof useTaskToolbar>;
type Translate = (key: TranslationKey, params?: TranslationParams) => string;

interface TaskBulkMessageApi {
  success: (content: string) => unknown;
  warning: (content: string) => unknown;
  error: (content: string) => unknown;
}

interface UseTaskBulkActionsOptions {
  taskStore: ReturnType<typeof useTaskStore>;
  toolbar: TaskToolbar;
  message: TaskBulkMessageApi;
  t: Translate;
}

export function useTaskBulkActions({ taskStore, toolbar, message, t }: UseTaskBulkActionsOptions) {
  const showBulkDeleteConfirm = ref(false);
  const bulkDeleteMode = ref<"delete" | "clearTrash">("delete");
  const isBulkOperating = ref(false);
  const bulkDeleteTaskCount = computed(() =>
    bulkDeleteMode.value === "clearTrash"
      ? toolbar.clearTrashCandidates.value.length
      : toolbar.deleteCandidates.value.length,
  );

  async function pauseVisibleTasks() {
    await runBatch(toolbar.pauseCandidates.value, (task) => taskStore.pauseTask(task.id), "task.bulk.pauseSuccess", "task.bulk.noPauseable");
  }

  async function resumeVisibleTasks() {
    await runBatch(toolbar.resumeCandidates.value, (task) => taskStore.resumeTask(task.id), "task.bulk.resumeSuccess", "task.bulk.noResumable");
  }

  function requestDeleteVisibleTasks() {
    if (!toolbar.canDeleteVisible.value) {
      message.warning(t("task.bulk.noDeletable"));
      return;
    }
    bulkDeleteMode.value = "delete";
    showBulkDeleteConfirm.value = true;
  }

  function requestClearTrash() {
    if (!toolbar.canClearTrash.value) {
      message.warning(t("task.bulk.trashEmpty"));
      return;
    }
    bulkDeleteMode.value = "clearTrash";
    showBulkDeleteConfirm.value = true;
  }

  async function confirmDeleteVisibleTasks() {
    try {
      if (bulkDeleteMode.value === "clearTrash") {
        await runBatch(toolbar.clearTrashCandidates.value, (task) => taskStore.permanentlyDeleteTask(task.id), "task.bulk.clearTrashSuccess", "task.bulk.trashEmpty");
        return;
      }
      await runBatch(toolbar.deleteCandidates.value, (task) => taskStore.deleteTask(task.id, false), "task.bulk.deleteSuccess", "task.bulk.noDeletable");
    } finally {
      showBulkDeleteConfirm.value = false;
    }
  }

  async function runBatch(
    candidates: DownloadTask[],
    operation: (task: DownloadTask) => Promise<unknown>,
    successKey: TranslationKey,
    emptyKey: TranslationKey,
  ) {
    if (candidates.length === 0) {
      message.warning(t(emptyKey));
      return;
    }
    isBulkOperating.value = true;
    try {
      const result = await runTaskToolbarBatch(candidates, operation);
      if (result.successCount > 0) message.success(t(successKey, { count: result.successCount }));
      if (result.failureCount > 0) message.error(t("task.bulk.partialFailed", { count: result.failureCount }));
    } finally {
      isBulkOperating.value = false;
    }
  }

  return {
    showBulkDeleteConfirm,
    bulkDeleteMode,
    isBulkOperating,
    bulkDeleteTaskCount,
    pauseVisibleTasks,
    resumeVisibleTasks,
    requestDeleteVisibleTasks,
    requestClearTrash,
    confirmDeleteVisibleTasks,
  };
}
