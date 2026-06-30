import { onBeforeUnmount, onMounted } from "vue";
import { useTaskStore } from "../stores/taskStore";

interface UseTaskPollingOptions {
  intervalMs?: number;
  onRefreshError?: (message: string) => void;
  onTaskError?: (message: string) => void;
}

export function useTaskPolling(options: UseTaskPollingOptions = {}) {
  const taskStore = useTaskStore();
  let timer: number | undefined;

  async function refresh(showError = false) {
    const result = await taskStore.refreshTasks({ showError });
    if (result.refreshError) {
      options.onRefreshError?.(result.refreshError);
    }
    for (const taskErrorMessage of result.taskErrorMessages) {
      options.onTaskError?.(taskErrorMessage);
    }
  }

  function start() {
    if (timer) {
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

  onMounted(() => {
    void refresh();
    start();
  });

  onBeforeUnmount(stop);

  return {
    refresh,
    start,
    stop,
  };
}
