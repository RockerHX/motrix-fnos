import { defineStore } from "pinia";
import { ref } from "vue";
import {
  createDownloadTask,
  deleteDownloadTask,
  listDownloadTasks,
  pauseDownloadTask,
  redownloadDownloadTask,
  resumeDownloadTask,
} from "../services/taskService";
import type { RuntimeExitingPayload, TasksSnapshotPayload } from "../../../services/runtimeEvents";
import type { CreateDownloadTaskRequest, DownloadTask } from "../../../types/tasks";

interface RefreshTasksOptions {
  showError?: boolean;
}

interface RefreshTasksResult {
  refreshError?: string;
  taskErrorMessages: string[];
}

export const useTaskStore = defineStore("tasks", () => {
  const tasks = ref<DownloadTask[]>([]);
  const isCreating = ref(false);
  const isRefreshing = ref(false);
  const operatingTaskIds = ref<number[]>([]);
  const lastRefreshErrorAt = ref(0);
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
        return { refreshError: getErrorMessage(error), taskErrorMessages: [] };
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

  async function runTaskOperation(
    taskId: number,
    operation: () => Promise<DownloadTask>,
  ): Promise<DownloadTask> {
    ensureRuntimeActive();
    beginTaskOperation(taskId);
    try {
      const task = await operation();
      if (!isRuntimeExiting.value) {
        upsertTask(task);
        await refreshTasks({ showError: true });
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
    runtimeExitReason.value = payload.reason || "应用正在退出";
  }

  function ensureRuntimeActive() {
    if (isRuntimeExiting.value) {
      throw new Error("应用正在退出，请稍候");
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
        messages.push(`任务下载失败：${formatTaskError(task)}`);
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
    isCreating,
    isRefreshing,
    operatingTaskIds,
    pendingTaskErrorMessages,
    isRuntimeExiting,
    runtimeExitReason,
    createTask,
    pauseTask,
    resumeTask,
    redownloadTask,
    deleteTask,
    refreshTasks,
    applyTaskSnapshot,
    markRuntimeExiting,
    consumeTaskErrorMessages,
    isTaskOperating,
  };
});

function taskKey(task: DownloadTask) {
  return task.gid || String(task.id);
}

function formatTaskError(task: DownloadTask) {
  const code = task.errorCode ? `错误码 ${task.errorCode}：` : "";
  return `${code}${task.errorMessage || "未知错误"}`;
}

function getErrorMessage(error: unknown) {
  if (error instanceof Error) {
    return error.message;
  }

  const message = String(error);
  return message || "操作失败，请稍后重试";
}
