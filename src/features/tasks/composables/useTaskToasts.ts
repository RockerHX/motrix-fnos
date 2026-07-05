import { watch } from "vue";
import { useTaskStore } from "../stores/taskStore";

interface TaskToastMessageApi {
  error: (content: string) => unknown;
}

interface UseTaskToastsOptions {
  taskStore: ReturnType<typeof useTaskStore>;
  message: TaskToastMessageApi;
}

export function useTaskToasts({ taskStore, message }: UseTaskToastsOptions) {
  async function refreshTasks(showError = false) {
    const result = await taskStore.refreshTasks({ showError });
    if (result.refreshError) {
      message.error(result.refreshError);
    }
    flushTaskErrorMessages();
  }

  async function refreshRemovedTasks(showError = false) {
    const result = await taskStore.refreshRemovedTasks({ showError });
    if (result.refreshError) {
      message.error(result.refreshError);
    }
  }

  function flushTaskErrorMessages() {
    for (const errorMessage of taskStore.consumeTaskErrorMessages()) {
      message.error(errorMessage);
    }
  }

  watch(
    () => taskStore.pendingTaskErrorMessages.length,
    (count) => {
      if (count > 0) {
        flushTaskErrorMessages();
      }
    },
  );

  return {
    refreshTasks,
    refreshRemovedTasks,
    flushTaskErrorMessages,
  };
}
