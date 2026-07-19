import { getErrorMessage } from "../../../app/utils/errors";
import type { TranslationKey, TranslationParams } from "../../../i18n";
import type { DownloadTask } from "../../../types/tasks";
import type { useTaskStore } from "../stores/taskStore";

type TaskStore = ReturnType<typeof useTaskStore>;
type Translate = (key: TranslationKey, params?: TranslationParams) => string;

interface TaskStatusMessageApi {
  success: (content: string) => unknown;
  warning: (content: string) => unknown;
  error: (content: string) => unknown;
}

interface UseTaskStatusActionsOptions {
  taskStore: TaskStore;
  message: TaskStatusMessageApi;
  t: Translate;
}

const INTERACTIVE_TARGET_SELECTOR =
  "button, a, input, textarea, select, summary, [role='button'], [role='link'], [contenteditable='true']";

export function useTaskStatusActions({ taskStore, message, t }: UseTaskStatusActionsOptions) {
  async function pauseTask(task: DownloadTask) {
    if (!ensureCanOperate(task)) return;
    try {
      await taskStore.pauseTask(task.id);
      message.success(t("task.actions.paused"));
    } catch (error) {
      message.error(getErrorMessage(error, t("task.operationFailed")));
    }
  }

  async function resumeTask(task: DownloadTask) {
    if (!ensureCanOperate(task)) return;
    try {
      await taskStore.resumeTask(task.id);
      message.success(t("task.actions.resumed"));
    } catch (error) {
      message.error(getErrorMessage(error, t("task.operationFailed")));
    }
  }

  async function handleTaskDoubleClick(task: DownloadTask, event: MouseEvent) {
    if (isInteractiveTarget(event.target) || taskStore.isTaskOperating(task.id)) return;

    if (task.status === "active" || task.status === "pending") {
      await pauseTask(task);
      return;
    }

    if (!task.confirmationRequired && (task.status === "paused" || task.status === "error")) {
      await resumeTask(task);
    }
  }

  function ensureCanOperate(task: DownloadTask) {
    if (taskStore.isTaskOperating(task.id)) return false;
    if (taskStore.isRuntimeExiting) {
      message.warning(t("task.runtimeExiting"));
      return false;
    }
    return true;
  }

  return {
    pauseTask,
    resumeTask,
    handleTaskDoubleClick,
  };
}

function isInteractiveTarget(target: EventTarget | null) {
  return target instanceof Element && target.closest(INTERACTIVE_TARGET_SELECTOR) !== null;
}
