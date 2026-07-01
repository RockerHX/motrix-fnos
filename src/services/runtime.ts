import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useTaskStore } from "../features/tasks/stores/taskStore";

export const RUNTIME_EXITING_EVENT = "runtime://exiting";

export interface RuntimeExitingPayload {
  reason: string;
  timestamp: number;
}

let unlistenPromise: Promise<UnlistenFn> | null = null;

export function initializeRuntimeEventListeners() {
  if (unlistenPromise) {
    return unlistenPromise;
  }

  unlistenPromise = listen<RuntimeExitingPayload>(RUNTIME_EXITING_EVENT, (event) => {
    useTaskStore().markRuntimeExiting(event.payload);
  });

  return unlistenPromise;
}

export async function disposeRuntimeEventListeners() {
  const unlisten = await unlistenPromise;
  unlisten?.();
  unlistenPromise = null;
}
