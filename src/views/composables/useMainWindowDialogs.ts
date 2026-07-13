import { ref } from "vue";
import { useTaskStore } from "../../features/tasks/stores/taskStore";
import { useTaskToolbar } from "../../features/tasks/composables/useTaskToolbar";
import type { TranslationKey, TranslationParams } from "../../i18n";

interface UseMainWindowDialogsOptions {
  taskStore: ReturnType<typeof useTaskStore>;
  toolbar: ReturnType<typeof useTaskToolbar>;
  message: { warning: (content: string) => unknown };
  t: (key: TranslationKey, params?: TranslationParams) => string;
}

export function useMainWindowDialogs({ taskStore, toolbar, message, t }: UseMainWindowDialogsOptions) {
  const showCreateDialog = ref(false);
  const showAbout = ref(false);
  const showDiagnostics = ref(false);
  const showHelp = ref(false);
  const showSettings = ref(false);

  function openCreateDialog() {
    if (taskStore.isRuntimeExiting) {
      message.warning(t("task.runtimeExiting"));
      return;
    }
    showCreateDialog.value = true;
  }

  function handleToolbarCreate() {
    if (!toolbar.canCreate.value) {
      if (taskStore.isRuntimeExiting) message.warning(t("task.runtimeExiting"));
      return;
    }
    openCreateDialog();
  }

  return { showCreateDialog, showAbout, showDiagnostics, showHelp, showSettings, openCreateDialog, handleToolbarCreate };
}
