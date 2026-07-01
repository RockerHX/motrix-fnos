import { onBeforeUnmount, onMounted, watch } from "vue";
import { useTaskStore } from "../stores/taskStore";

interface UseTaskPollingOptions {
  intervalMs?: number;
  onRefreshError?: (message: string) => void;
  onTaskError?: (message: string) => void;
}

export function useTaskPolling(options: UseTaskPollingOptions = {}) {
  const taskStore = useTaskStore();
  let timer: number | undefined;
  let refreshVersion = 0;

  async function refresh(showError = false) {
    if (taskStore.isRuntimeExiting) {
      return;
    }

    const currentVersion = ++refreshVersion;
    const result = await taskStore.refreshTasks({ showError });
    if (taskStore.isRuntimeExiting || currentVersion !== refreshVersion) {
      return;
    }
    if (result.refreshError) {
      options.onRefreshError?.(result.refreshError);
    }
    for (const taskErrorMessage of result.taskErrorMessages) {
      options.onTaskError?.(taskErrorMessage);
    }
  }

  function start() {
    if (timer || taskStore.isRuntimeExiting) {
      return;
    }

    timer = window.setInterval(() => {
      void refresh();
    }, options.intervalMs ?? 2000);
  }

  function stop() {
    if (!timer) {
      return;
    }

    window.clearInterval(timer);
    timer = undefined;
  }

  watch(
    () => taskStore.isRuntimeExiting,
    (isExiting) => {
      if (isExiting) {
        refreshVersion += 1;
        stop();
      }
    },
  );

  onMounted(() => {
    if (!taskStore.isRuntimeExiting) {
      void refresh();
      start();
    }
  });

  onBeforeUnmount(stop);

  return {
    refresh,
    start,
    stop,
  };
}
