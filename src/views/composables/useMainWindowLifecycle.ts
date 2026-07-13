import { onMounted, watch, type Ref } from "vue";

interface UseMainWindowLifecycleOptions {
  errorMessage: Ref<string>;
  isRuntimeExiting: Ref<boolean>;
  showCreateDialog: Ref<boolean>;
  message: { error: (content: string) => unknown };
  refreshTasks: (showError?: boolean) => Promise<void>;
  refreshAria2Status: () => Promise<unknown>;
}

export function useMainWindowLifecycle(options: UseMainWindowLifecycleOptions) {
  const { errorMessage, isRuntimeExiting, showCreateDialog, message, refreshTasks, refreshAria2Status } = options;

  watch(errorMessage, (nextMessage) => {
    if (nextMessage) message.error(nextMessage);
  });
  watch(isRuntimeExiting, (exiting) => {
    if (exiting) showCreateDialog.value = false;
  });
  onMounted(() => {
    void refreshAria2Status();
    void refreshTasks(true);
  });
}
