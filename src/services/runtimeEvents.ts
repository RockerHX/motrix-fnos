import { useTaskStore } from "../features/tasks/stores/taskStore";
import type { DownloadTask } from "../types/tasks";
import type { AuthStatus } from "../features/auth/types";

export interface RuntimeExitingPayload {
  reason: string;
  timestamp: number;
}

export interface TasksSnapshotPayload {
  tasks: DownloadTask[];
}

let eventSource: EventSource | null = null;
let retryTimer: ReturnType<typeof setTimeout> | null = null;
let retryAttempt = 0;
let generation = 0;
let options: RuntimeEventOptions = defaultOptions();
const RETRY_DELAYS_SECONDS = [1, 2, 4, 8, 16, 30];

export interface RuntimeEventOptions {
  checkAuth: () => Promise<AuthStatus>;
  onUnauthorized: (status: AuthStatus) => void | Promise<void>;
}

export function initializeRuntimeEvents(nextOptions: RuntimeEventOptions = defaultOptions()) {
  options = nextOptions;
  if (eventSource) {
    return eventSource;
  }
  clearRetryTimer();
  generation += 1;
  return connect(generation);
}

function connect(currentGeneration: number) {
  const source = new EventSource("/api/events");
  const taskStore = useTaskStore();

  source.addEventListener("open", () => {
    retryAttempt = 0;
  });

  source.addEventListener("tasks.snapshot", (event) => {
    const payload = parseEventPayload<TasksSnapshotPayload>(event);
    if (payload && !taskStore.isRuntimeExiting) {
      taskStore.applyTaskSnapshot(payload);
    }
  });

  source.addEventListener("runtime.exiting", (event) => {
    const payload = parseEventPayload<RuntimeExitingPayload>(event);
    if (payload) {
      taskStore.markRuntimeExiting(payload);
    }
  });

  source.addEventListener("error", () => {
    void handleSourceError(source, currentGeneration);
  });

  eventSource = source;
  return eventSource;
}

export function disposeRuntimeEvents() {
  generation += 1;
  clearRetryTimer();
  eventSource?.close();
  eventSource = null;
  retryAttempt = 0;
}

async function handleSourceError(source: EventSource, currentGeneration: number) {
  if (source !== eventSource || currentGeneration !== generation) return;
  source.close();
  eventSource = null;
  try {
    const status = await options.checkAuth();
    if (currentGeneration !== generation) return;
    const hasAccess = !status.setupRequired && (!status.enabled || status.authenticated);
    if (!hasAccess) {
      await options.onUnauthorized(status);
      return;
    }
  } catch {
    if (currentGeneration !== generation) return;
  }
  scheduleReconnect(currentGeneration);
}

function scheduleReconnect(currentGeneration: number) {
  if (retryTimer || currentGeneration !== generation) return;
  const delay = RETRY_DELAYS_SECONDS[Math.min(retryAttempt, RETRY_DELAYS_SECONDS.length - 1)] * 1_000;
  retryAttempt += 1;
  retryTimer = setTimeout(() => {
    retryTimer = null;
    if (currentGeneration === generation && !eventSource) connect(currentGeneration);
  }, delay);
}

function clearRetryTimer() {
  if (retryTimer) clearTimeout(retryTimer);
  retryTimer = null;
}

function defaultOptions(): RuntimeEventOptions {
  return {
    checkAuth: async () => ({ setupRequired: false, enabled: false, authenticated: false, csrfToken: null }),
    onUnauthorized: () => undefined,
  };
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
