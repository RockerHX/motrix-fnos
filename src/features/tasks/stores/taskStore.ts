import { defineStore } from "pinia";
import { ref } from "vue";
import {
  createBatchDownloadTasks,
  createDownloadTask,
  createTorrentDownloadTask,
  confirmDownloadTaskFiles,
  deleteDownloadTask,
  listDownloadTasks,
  listRemovedDownloadTasks,
  pauseDownloadTask,
  permanentlyDeleteDownloadTask,
  redownloadDownloadTask,
  resumeDownloadTask,
  restoreDownloadTask,
  updateDownloadTaskProxy,
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
  let tasksRequestGeneration = 0;
  let removedTasksRequestGeneration = 0;
  let tasksRequestController: AbortController | null = null;
  let removedTasksRequestController: AbortController | null = null;
  let isTasksRequestInFlight = false;
  let isRemovedTasksRequestInFlight = false;
  let latestTasksSnapshotRevision = -1;

  async function refreshTasks(options: RefreshTasksOptions = {}): Promise<RefreshTasksResult> {
    if (isRuntimeExiting.value) {
      return { taskErrorMessages: [] };
    }

    const request = beginTasksRequest();
    try {
      const nextTasks = await listDownloadTasks(request.controller.signal);
      if (!isCurrentTasksRequest(request) || isRuntimeExiting.value) {
        return { taskErrorMessages: [] };
      }
      const taskErrorMessages = hasLoadedTasks.value
        ? collectNewTaskErrorMessages(tasks.value, nextTasks)
        : [];
      applyResolvedTasks(nextTasks, taskErrorMessages);
      return { taskErrorMessages };
    } catch (error) {
      if (!isCurrentTasksRequest(request) || isRuntimeExiting.value) {
        return { taskErrorMessages: [] };
      }
      const now = Date.now();
      // 时间窗口只抑制自动刷新产生的重复提示；刷新请求仍会照常执行，用户主动刷新也始终返回错误。
      const shouldReport = options.showError || now - lastRefreshErrorAt.value > 10000;
      if (shouldReport) {
        lastRefreshErrorAt.value = now;
        return { refreshError: getErrorMessage(error, t("task.operationFailed")), taskErrorMessages: [] };
      }
      return { taskErrorMessages: [] };
    } finally {
      finishTasksRequest(request);
    }
  }

  async function refreshRemovedTasks(options: RefreshTasksOptions = {}): Promise<RefreshTasksResult> {
    if (isRuntimeExiting.value) {
      return { taskErrorMessages: [] };
    }

    const request = beginRemovedTasksRequest();
    try {
      const nextRemovedTasks = await listRemovedDownloadTasks(request.controller.signal);
      if (!isCurrentRemovedTasksRequest(request) || isRuntimeExiting.value) {
        return { taskErrorMessages: [] };
      }
      removedTasks.value = nextRemovedTasks;
      return { taskErrorMessages: [] };
    } catch (error) {
      if (!isCurrentRemovedTasksRequest(request) || isRuntimeExiting.value) {
        return { taskErrorMessages: [] };
      }
      const now = Date.now();
      const shouldReport = options.showError || now - lastRemovedRefreshErrorAt.value > 10000;
      if (shouldReport) {
        lastRemovedRefreshErrorAt.value = now;
        return { refreshError: getErrorMessage(error, t("task.operationFailed")), taskErrorMessages: [] };
      }
      return { taskErrorMessages: [] };
    } finally {
      finishRemovedTasksRequest(request);
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
      // upsertTask 会把新任务插到列表头部，因此反向处理才能保持后端 created 数组的原始顺序。
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

  async function confirmTaskFiles(taskId: number, selectedFileIndexes: number[]): Promise<DownloadTask> {
    return runTaskOperation(taskId, () =>
      confirmDownloadTaskFiles(taskId, {
        selectedFileIndexes,
      }),
    );
  }

  async function updateTaskProxy(taskId: number, enabled: boolean): Promise<DownloadTask> {
    return runTaskOperation(taskId, () => updateDownloadTaskProxy(taskId, enabled));
  }

  async function redownloadTask(taskId: number, useProxy?: boolean): Promise<DownloadTask> {
    return runTaskOperation(taskId, () => redownloadDownloadTask(taskId, useProxy));
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

  async function restoreTask(taskId: number, useProxy?: boolean): Promise<DownloadTask> {
    ensureRuntimeActive();
    beginTaskOperation(taskId);
    try {
      const task = await restoreDownloadTask(taskId, useProxy);
      if (!isRuntimeExiting.value) {
        removedTasks.value = removedTasks.value.filter((item) => item.id !== taskId);
        upsertTask(task);
      }
      return task;
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
      // 退出事件可能先于 HTTP 响应到达；此时不能再用迟到结果覆盖已经锁定的退出态界面。
      if (!isRuntimeExiting.value) {
        if (task.status === "removed") {
          removeTask(task.id);
          upsertRemovedTask(task);
        } else {
          removedTasks.value = removedTasks.value.filter((item) => item.id !== task.id);
          upsertTask(task);
        }
      }
      return task;
    } finally {
      endTaskOperation(taskId);
    }
  }

  function applyTaskSnapshot(payload: TasksSnapshotPayload) {
    if (
      isRuntimeExiting.value ||
      !Number.isSafeInteger(payload.revision) ||
      payload.revision < latestTasksSnapshotRevision
    ) {
      return;
    }

    if (payload.revision > latestTasksSnapshotRevision) {
      latestTasksSnapshotRevision = payload.revision;
      cancelTasksRequest();
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

  function upsertRemovedTask(task: DownloadTask) {
    const existingIndex = removedTasks.value.findIndex((item) => item.id === task.id);
    if (existingIndex < 0) {
      removedTasks.value = [task, ...removedTasks.value];
      return;
    }

    removedTasks.value = removedTasks.value.map((item) => (item.id === task.id ? task : item));
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

  function clearSensitiveState() {
    cancelRefreshRequests();
    tasks.value = [];
    removedTasks.value = [];
    isCreating.value = false;
    isRefreshing.value = false;
    operatingTaskIds.value = [];
    lastRefreshErrorAt.value = 0;
    lastRemovedRefreshErrorAt.value = 0;
    hasLoadedTasks.value = false;
    pendingTaskErrorMessages.value = [];
    isRuntimeExiting.value = false;
    runtimeExitReason.value = "";
    latestTasksSnapshotRevision = -1;
    notifiedErrorTaskKeys.clear();
  }

  function cancelRefreshRequests() {
    cancelTasksRequest();
    cancelRemovedTasksRequest();
  }

  function beginTasksRequest() {
    cancelTasksRequest();
    const request = {
      generation: ++tasksRequestGeneration,
      controller: new AbortController(),
    };
    tasksRequestController = request.controller;
    isTasksRequestInFlight = true;
    syncRefreshing();
    return request;
  }

  function beginRemovedTasksRequest() {
    cancelRemovedTasksRequest();
    const request = {
      generation: ++removedTasksRequestGeneration,
      controller: new AbortController(),
    };
    removedTasksRequestController = request.controller;
    isRemovedTasksRequestInFlight = true;
    syncRefreshing();
    return request;
  }

  function isCurrentTasksRequest(request: { generation: number; controller: AbortController }) {
    return tasksRequestGeneration === request.generation && tasksRequestController === request.controller;
  }

  function isCurrentRemovedTasksRequest(request: { generation: number; controller: AbortController }) {
    return (
      removedTasksRequestGeneration === request.generation && removedTasksRequestController === request.controller
    );
  }

  function finishTasksRequest(request: { generation: number; controller: AbortController }) {
    if (!isCurrentTasksRequest(request)) return;
    tasksRequestController = null;
    isTasksRequestInFlight = false;
    syncRefreshing();
  }

  function finishRemovedTasksRequest(request: { generation: number; controller: AbortController }) {
    if (!isCurrentRemovedTasksRequest(request)) return;
    removedTasksRequestController = null;
    isRemovedTasksRequestInFlight = false;
    syncRefreshing();
  }

  function cancelTasksRequest() {
    tasksRequestGeneration += 1;
    tasksRequestController?.abort();
    tasksRequestController = null;
    isTasksRequestInFlight = false;
    syncRefreshing();
  }

  function cancelRemovedTasksRequest() {
    removedTasksRequestGeneration += 1;
    removedTasksRequestController?.abort();
    removedTasksRequestController = null;
    isRemovedTasksRequestInFlight = false;
    syncRefreshing();
  }

  function syncRefreshing() {
    isRefreshing.value = isTasksRequestInFlight || isRemovedTasksRequestInFlight;
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
    updateTaskProxy,
    confirmTaskFiles,
    redownloadTask,
    deleteTask,
    permanentlyDeleteTask,
    restoreTask,
    refreshTasks,
    refreshRemovedTasks,
    applyTaskSnapshot,
    markRuntimeExiting,
    consumeTaskErrorMessages,
    clearSensitiveState,
    cancelRefreshRequests,
    isTaskOperating,
  };
});

function taskKey(task: DownloadTask) {
  // 优先按 GID 去重，同一记录重新加入 Aria2 获得新 GID 后仍可提示新一轮失败；没有 GID 时才回退到稳定的应用任务 ID。
  return task.gid || String(task.id);
}
