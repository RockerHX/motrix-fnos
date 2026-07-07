import { defineStore } from "pinia";
import { ref } from "vue";
import {
  createBatchDownloadTasks,
  createDownloadTask,
  createTorrentDownloadTask,
  deleteDownloadTask,
  listDownloadTasks,
  listRemovedDownloadTasks,
  pauseDownloadTask,
  permanentlyDeleteDownloadTask,
  redownloadDownloadTask,
  resumeDownloadTask,
} from "../services/taskService";
import { t } from "../../../i18n";
import { getErrorMessage } from "../../../app/utils/errors";
import { formatTaskError } from "../utils/taskFormat";
import type { RuntimeExitingPayload, TasksSnapshotPayload } from "../../../services/runtimeEvents";
import type {
  CreateBatchDownloadTasksRequest,
  CreateBatchDownloadTasksResponse,
  CreateDownloadTaskRequest,
  CreateTorrentDownloadTaskRequest,
  DownloadTask,
} from "../../../types/tasks";

interface RefreshTasksOptions {
  showError?: boolean;
}

interface RefreshTasksResult {
  refreshError?: string;
  taskErrorMessages: string[];
}

export const useTaskStore = defineStore("tasks", () => {
  const tasks = ref<DownloadTask[]>([]);
  const removedTasks = ref<DownloadTask[]>([]);
  const isCreating = ref(false);
  const isRefreshing = ref(false);
  const operatingTaskIds = ref<number[]>([]);
  const lastRefreshErrorAt = ref(0);
  const lastRemovedRefreshErrorAt = ref(0);
  const notifiedErrorTaskKeys = new Set<string>();
  const hasLoadedTasks = ref(false);
  const pendingTaskErrorMessages = ref<string[]>([]);
  const isRuntimeExiting = ref(false);
  const runtimeExitReason = ref("");

  async function refreshTasks(options: RefreshTasksOptions = {}): Promise<RefreshTasksResult> {
    if (isRuntimeExiting.value) {
      return { taskErrorMessages: [] };
    }

    try {
      isRefreshing.value = true;
      const nextTasks = await listDownloadTasks();
      if (isRuntimeExiting.value) {
        return { taskErrorMessages: [] };
      }
      const taskErrorMessages = hasLoadedTasks.value
        ? collectNewTaskErrorMessages(tasks.value, nextTasks)
        : [];
      applyResolvedTasks(nextTasks, taskErrorMessages);
      return { taskErrorMessages };
    } catch (error) {
      const now = Date.now();
      const shouldReport = options.showError || now - lastRefreshErrorAt.value > 10000;
      if (shouldReport) {
        lastRefreshErrorAt.value = now;
        return { refreshError: getErrorMessage(error, t("task.operationFailed")), taskErrorMessages: [] };
      }
      return { taskErrorMessages: [] };
    } finally {
      isRefreshing.value = false;
    }
  }

  async function refreshRemovedTasks(options: RefreshTasksOptions = {}): Promise<RefreshTasksResult> {
    if (isRuntimeExiting.value) {
      return { taskErrorMessages: [] };
    }

    try {
      isRefreshing.value = true;
      removedTasks.value = await listRemovedDownloadTasks();
      return { taskErrorMessages: [] };
    } catch (error) {
      const now = Date.now();
      const shouldReport = options.showError || now - lastRemovedRefreshErrorAt.value > 10000;
      if (shouldReport) {
        lastRemovedRefreshErrorAt.value = now;
        return { refreshError: getErrorMessage(error, t("task.operationFailed")), taskErrorMessages: [] };
      }
      return { taskErrorMessages: [] };
    } finally {
      isRefreshing.value = false;
    }
  }

  async function createTask(payload: CreateDownloadTaskRequest): Promise<DownloadTask> {
    ensureRuntimeActive();
    isCreating.value = true;

    try {
      const task = await createDownloadTask(payload);
      tasks.value = [task, ...tasks.value.filter((item) => item.id !== task.id)];
      return task;
    } finally {
      isCreating.value = false;
    }
  }

  async function createBatchTasks(
    payload: CreateBatchDownloadTasksRequest,
  ): Promise<CreateBatchDownloadTasksResponse> {
    ensureRuntimeActive();
    isCreating.value = true;

    try {
      const result = await createBatchDownloadTasks(payload);
      for (const task of [...result.created].reverse()) {
        upsertTask(task);
      }
      return result;
    } finally {
      isCreating.value = false;
    }
  }

  async function createTorrentTask(payload: CreateTorrentDownloadTaskRequest): Promise<DownloadTask> {
    ensureRuntimeActive();
    isCreating.value = true;

    try {
      const task = await createTorrentDownloadTask(payload);
      upsertTask(task);
      return task;
    } finally {
      isCreating.value = false;
    }
  }

  async function pauseTask(taskId: number): Promise<DownloadTask> {
    return runTaskOperation(taskId, () => pauseDownloadTask(taskId));
  }

  async function resumeTask(taskId: number): Promise<DownloadTask> {
    return runTaskOperation(taskId, () => resumeDownloadTask(taskId));
  }

  async function redownloadTask(taskId: number): Promise<DownloadTask> {
    return runTaskOperation(taskId, () => redownloadDownloadTask(taskId));
  }

  async function deleteTask(taskId: number, deleteFiles: boolean): Promise<DownloadTask> {
    return runTaskOperation(taskId, () => deleteDownloadTask(taskId, deleteFiles));
  }

  async function permanentlyDeleteTask(taskId: number): Promise<void> {
    ensureRuntimeActive();
    beginTaskOperation(taskId);
    try {
      await permanentlyDeleteDownloadTask(taskId);
      removedTasks.value = removedTasks.value.filter((item) => item.id !== taskId);
    } finally {
      endTaskOperation(taskId);
    }
  }

  async function runTaskOperation(
    taskId: number,
    operation: () => Promise<DownloadTask>,
  ): Promise<DownloadTask> {
    ensureRuntimeActive();
    beginTaskOperation(taskId);
    try {
      const task = await operation();
      if (!isRuntimeExiting.value) {
        if (task.status === "removed") {
          removeTask(task.id);
        } else {
          upsertTask(task);
        }
      }
      return task;
    } finally {
      endTaskOperation(taskId);
    }
  }


  function applyTaskSnapshot(payload: TasksSnapshotPayload) {
    if (isRuntimeExiting.value) {
      return;
    }

    const nextTasks = payload.tasks;
    const taskErrorMessages = hasLoadedTasks.value
      ? collectNewTaskErrorMessages(tasks.value, nextTasks)
      : [];
    applyResolvedTasks(nextTasks, taskErrorMessages);
  }

  function markRuntimeExiting(payload: RuntimeExitingPayload) {
    isRuntimeExiting.value = true;
    runtimeExitReason.value = payload.reason || t("task.runtimeExiting");
  }

  function ensureRuntimeActive() {
    if (isRuntimeExiting.value) {
      throw new Error(t("task.runtimeExiting"));
    }
  }

  function isTaskOperating(taskId: number) {
    return operatingTaskIds.value.includes(taskId);
  }

  function beginTaskOperation(taskId: number) {
    if (!operatingTaskIds.value.includes(taskId)) {
      operatingTaskIds.value = [...operatingTaskIds.value, taskId];
    }
  }

  function endTaskOperation(taskId: number) {
    operatingTaskIds.value = operatingTaskIds.value.filter((id) => id !== taskId);
  }

  function upsertTask(task: DownloadTask) {
    const existingIndex = tasks.value.findIndex((item) => item.id === task.id);
    if (existingIndex < 0) {
      tasks.value = [task, ...tasks.value];
      return;
    }

    tasks.value = tasks.value.map((item) => (item.id === task.id ? task : item));
  }

  function removeTask(taskId: number) {
    tasks.value = tasks.value.filter((item) => item.id !== taskId);
  }

  function applyResolvedTasks(nextTasks: DownloadTask[], taskErrorMessages: string[]) {
    rememberErrorTasks(nextTasks);
    tasks.value = nextTasks;
    hasLoadedTasks.value = true;
    if (taskErrorMessages.length > 0) {
      pendingTaskErrorMessages.value = [...pendingTaskErrorMessages.value, ...taskErrorMessages];
    }
  }

  function consumeTaskErrorMessages() {
    const messages = [...pendingTaskErrorMessages.value];
    pendingTaskErrorMessages.value = [];
    return messages;
  }

  function collectNewTaskErrorMessages(previousTasks: DownloadTask[], nextTasks: DownloadTask[]) {
    const previousStatus = new Map(previousTasks.map((task) => [taskKey(task), task.status]));
    const messages: string[] = [];

    for (const task of nextTasks) {
      const key = taskKey(task);
      if (
        task.status === "error" &&
        previousStatus.get(key) !== "error" &&
        !notifiedErrorTaskKeys.has(key)
      ) {
        notifiedErrorTaskKeys.add(key);
        messages.push(t("task.failed", { message: formatTaskError(task) }));
      }
    }

    return messages;
  }

  function rememberErrorTasks(nextTasks: DownloadTask[]) {
    for (const task of nextTasks) {
      if (task.status === "error") {
        notifiedErrorTaskKeys.add(taskKey(task));
      }
    }
  }

  return {
    tasks,
    removedTasks,
    isCreating,
    isRefreshing,
    operatingTaskIds,
    pendingTaskErrorMessages,
    isRuntimeExiting,
    runtimeExitReason,
    createTask,
    createBatchTasks,
    createTorrentTask,
    pauseTask,
    resumeTask,
    redownloadTask,
    deleteTask,
    permanentlyDeleteTask,
    refreshTasks,
    refreshRemovedTasks,
    applyTaskSnapshot,
    markRuntimeExiting,
    consumeTaskErrorMessages,
    isTaskOperating,
  };
});

function taskKey(task: DownloadTask) {
  return task.gid || String(task.id);
}
