import { useTaskStore } from "../features/tasks/stores/taskStore";
import type { DownloadTask } from "../types/tasks";

export interface RuntimeExitingPayload {
  reason: string;
  timestamp: number;
}

export interface TasksSnapshotPayload {
  tasks: DownloadTask[];
}

let eventSource: EventSource | null = null;

export function initializeRuntimeEvents() {
  if (eventSource) {
    return eventSource;
  }

  const source = new EventSource("/api/events");
  const taskStore = useTaskStore();

  source.addEventListener("tasks.snapshot", () => {
    if (!taskStore.isRuntimeExiting) {
      void taskStore.refreshTasks();
    }
  });

  source.addEventListener("runtime.exiting", (event) => {
    const payload = parseEventPayload<RuntimeExitingPayload>(event);
    if (payload) {
      taskStore.markRuntimeExiting(payload);
    }
  });

  eventSource = source;
  return eventSource;
}

export function disposeRuntimeEvents() {
  eventSource?.close();
  eventSource = null;
}

function parseEventPayload<T>(event: Event): T | null {
  if (!(event instanceof MessageEvent) || typeof event.data !== "string") {
    return null;
  }

  try {
    return JSON.parse(event.data) as T;
  } catch {
    return null;
  }
}
