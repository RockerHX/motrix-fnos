import { defineStore } from "pinia";
import { ref } from "vue";
import { createDownloadTask, listDownloadTasks } from "../services/taskService";
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
  const lastRefreshErrorAt = ref(0);
  const notifiedErrorTaskKeys = new Set<string>();

  async function refreshTasks(options: RefreshTasksOptions = {}): Promise<RefreshTasksResult> {
    try {
      isRefreshing.value = true;
      const nextTasks = await listDownloadTasks();
      const taskErrorMessages = collectNewTaskErrorMessages(tasks.value, nextTasks);
      tasks.value = nextTasks;
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
    isCreating.value = true;

    try {
      const task = await createDownloadTask(payload);
      tasks.value = [task, ...tasks.value.filter((item) => item.id !== task.id)];
      return task;
    } finally {
      isCreating.value = false;
    }
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

  return {
    tasks,
    isCreating,
    isRefreshing,
    createTask,
    refreshTasks,
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
