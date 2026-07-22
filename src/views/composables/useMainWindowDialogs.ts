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
  const showJsonRpcGuide = ref(false);

  function closeSecondaryDialogs() {
    showAbout.value = false;
    showDiagnostics.value = false;
    showHelp.value = false;
    showSettings.value = false;
    showJsonRpcGuide.value = false;
  }

  function openAbout() {
    closeSecondaryDialogs();
    showAbout.value = true;
  }

  function openDiagnostics() {
    closeSecondaryDialogs();
    showDiagnostics.value = true;
  }

  function openHelp() {
    closeSecondaryDialogs();
    showHelp.value = true;
  }

  function openSettings() {
    closeSecondaryDialogs();
    showSettings.value = true;
  }

  function openJsonRpcGuide() {
    closeSecondaryDialogs();
    showJsonRpcGuide.value = true;
  }

  function openSettingsFromJsonRpcGuide() {
    closeSecondaryDialogs();
    showSettings.value = true;
  }

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

  return {
    showCreateDialog,
    showAbout,
    showDiagnostics,
    showHelp,
    showSettings,
    showJsonRpcGuide,
    openAbout,
    openDiagnostics,
    openHelp,
    openSettings,
    openJsonRpcGuide,
    openSettingsFromJsonRpcGuide,
    openCreateDialog,
    handleToolbarCreate,
  };
}
