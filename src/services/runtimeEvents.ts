import { useTaskStore } from "../features/tasks/stores/taskStore";
import type { DownloadTask } from "../types/tasks";
import type { AuthStatus } from "../features/auth/types";

export interface RuntimeExitingPayload {
  reason: string;
  timestamp: number;
}

export interface TasksSnapshotPayload {
  revision: number;
  tasks: DownloadTask[];
}

let eventController: AbortController | null = null;
let retryTimer: ReturnType<typeof setTimeout> | null = null;
let authTimer: ReturnType<typeof setInterval> | null = null;
let retryAttempt = 0;
let generation = 0;
let hasOpenedConnection = false;
let options: RuntimeEventOptions = defaultOptions();
const RETRY_DELAYS_SECONDS = [1, 2, 4, 8, 16, 30];
const AUTH_RECHECK_INTERVAL_MS = 15_000;

export interface RuntimeEventOptions {
  checkAuth: () => Promise<AuthStatus>;
  onUnauthorized: (status: AuthStatus) => void | Promise<void>;
  getAccessToken?: () => string | null;
}

export function initializeRuntimeEvents(nextOptions: RuntimeEventOptions = defaultOptions()) {
  options = nextOptions;
  if (eventController) return eventController;
  clearRetryTimer();
  generation += 1;
  hasOpenedConnection = false;
  const controller = new AbortController();
  eventController = controller;
  void connect(controller, generation);
  return controller;
}

async function connect(controller: AbortController, currentGeneration: number) {
  if (currentGeneration !== generation || eventController !== controller) return;
  const token = options.getAccessToken?.() ?? null;
  const headers: HeadersInit = token ? { Authorization: `Bearer ${token}` } : {};

  let response: Response;
  try {
    response = await fetch("/api/events", {
      method: "GET",
      credentials: "omit",
      headers,
      signal: controller.signal,
    });
  } catch {
    if (!controller.signal.aborted) await handleConnectionFailure(controller, currentGeneration);
    return;
  }

  if (controller.signal.aborted || currentGeneration !== generation || eventController !== controller) return;
  if (!response.ok || !response.body) {
    await handleConnectionFailure(controller, currentGeneration);
    return;
  }

  markConnectionOpened(currentGeneration);
  authTimer = setInterval(() => {
    void recheckAuth(controller, currentGeneration);
  }, AUTH_RECHECK_INTERVAL_MS);

  try {
    await readSseStream(response.body, controller, currentGeneration);
  } catch {
    if (!controller.signal.aborted) {
      await handleConnectionFailure(controller, currentGeneration);
    }
  } finally {
    clearAuthTimer();
  }

  if (!controller.signal.aborted && currentGeneration === generation && eventController === controller) {
    await handleConnectionFailure(controller, currentGeneration);
  }
}

function markConnectionOpened(currentGeneration: number) {
  const taskStore = useTaskStore();
  retryAttempt = 0;
  const reconnected = hasOpenedConnection;
  hasOpenedConnection = true;
  if (reconnected && !taskStore.isRuntimeExiting) {
    void taskStore.refreshTasks();
  }
  if (currentGeneration !== generation) return;
}

async function readSseStream(body: ReadableStream<Uint8Array>, controller: AbortController, currentGeneration: number) {
  const reader = body.getReader();
  const decoder = new TextDecoder();
  let buffer = "";
  let eventName = "message";
  let dataLines: string[] = [];
  const cancelReader = () => {
    void reader.cancel().catch(() => undefined);
  };
  controller.signal.addEventListener("abort", cancelReader, { once: true });

  const dispatch = () => {
    if (!dataLines.length) {
      eventName = "message";
      return;
    }
    const data = dataLines.join("\n");
    handleSseEvent(eventName, data, currentGeneration);
    eventName = "message";
    dataLines = [];
  };

  try {
    while (!controller.signal.aborted && currentGeneration === generation) {
      const result = await reader.read();
      if (result.done) {
        buffer += decoder.decode();
        break;
      }
      buffer += decoder.decode(result.value, { stream: true });
      const lines = buffer.split(/\r?\n/);
      buffer = lines.pop() ?? "";
      for (const line of lines) {
        if (!line) {
          dispatch();
        } else if (line.startsWith("event:")) {
          eventName = line.slice(6).trim() || "message";
        } else if (line.startsWith("data:")) {
          const value = line.slice(5);
          dataLines.push(value.startsWith(" ") ? value.slice(1) : value);
        }
      }
    }
    if (buffer) {
      if (buffer.startsWith("data:")) dataLines.push(buffer.slice(5).replace(/^ /, ""));
      dispatch();
    }
  } finally {
    controller.signal.removeEventListener("abort", cancelReader);
    reader.releaseLock();
  }
}

function handleSseEvent(eventName: string, data: string, currentGeneration: number) {
  if (currentGeneration !== generation) return;
  const taskStore = useTaskStore();
  if (eventName === "tasks.snapshot") {
    const payload = parseEventPayload<TasksSnapshotPayload>(data);
    if (payload && !taskStore.isRuntimeExiting) taskStore.applyTaskSnapshot(payload);
  } else if (eventName === "runtime.exiting") {
    const payload = parseEventPayload<RuntimeExitingPayload>(data);
    if (payload) taskStore.markRuntimeExiting(payload);
  }
}

async function recheckAuth(controller: AbortController, currentGeneration: number) {
  if (controller.signal.aborted || currentGeneration !== generation) return;
  try {
    const status = await options.checkAuth();
    if (currentGeneration !== generation || controller.signal.aborted) return;
    if (!hasAccess(status)) {
      await options.onUnauthorized(status);
      controller.abort();
      clearConnection(controller);
    }
  } catch {
    // A transient status failure is handled by the stream/reconnect path.
  }
}

async function handleConnectionFailure(controller: AbortController, currentGeneration: number) {
  if (currentGeneration !== generation || eventController !== controller) return;
  clearConnection(controller);
  try {
    const status = await options.checkAuth();
    if (currentGeneration !== generation) return;
    if (!hasAccess(status)) {
      await options.onUnauthorized(status);
      return;
    }
  } catch {
    if (currentGeneration !== generation) return;
  }
  scheduleReconnect(currentGeneration);
}

function hasAccess(status: AuthStatus) {
  return !status.setupRequired && (!status.enabled || status.authenticated);
}

export function disposeRuntimeEvents() {
  generation += 1;
  clearRetryTimer();
  clearAuthTimer();
  eventController?.abort();
  eventController = null;
  retryAttempt = 0;
  hasOpenedConnection = false;
  useTaskStore().cancelRefreshRequests();
}

function clearConnection(controller: AbortController) {
  if (eventController === controller) eventController = null;
  clearAuthTimer();
}

function scheduleReconnect(currentGeneration: number) {
  if (retryTimer || currentGeneration !== generation) return;
  const delay = RETRY_DELAYS_SECONDS[Math.min(retryAttempt, RETRY_DELAYS_SECONDS.length - 1)] * 1_000;
  retryAttempt += 1;
  retryTimer = setTimeout(() => {
    retryTimer = null;
    if (currentGeneration === generation && !eventController) {
      const controller = new AbortController();
      eventController = controller;
      void connect(controller, currentGeneration);
    }
  }, delay);
}

function clearRetryTimer() {
  if (retryTimer) clearTimeout(retryTimer);
  retryTimer = null;
}

function clearAuthTimer() {
  if (authTimer) clearInterval(authTimer);
  authTimer = null;
}

function defaultOptions(): RuntimeEventOptions {
  return {
    checkAuth: async () => ({ setupRequired: false, enabled: false, authenticated: false }),
    onUnauthorized: () => undefined,
    getAccessToken: () => null,
  };
}

function parseEventPayload<T>(data: string): T | null {
  try {
    return JSON.parse(data) as T;
  } catch {
    return null;
  }
}
