import { computed, reactive, ref, watch, type Ref } from "vue";
import { useDownloadProxyStore } from "../../settings/stores/downloadProxyStore";
import {
  createTaskCreateFormState,
  resetTaskCreateFormState,
  type TaskCreateInputType,
} from "./taskCreateFormModel";
import { useTaskCreateValidation } from "./useTaskCreateValidation";
import { useTaskSaveDirectory } from "./useTaskSaveDirectory";
import { useTaskCreateSubmission } from "./useTaskCreateSubmission";

interface UseTaskCreateFormOptions {
  show: Ref<boolean>;
  onClose: () => void;
  onCreated: () => void;
  onOpenProxySettings: () => void;
}

export function useTaskCreateForm({ show, onClose, onCreated, onOpenProxySettings }: UseTaskCreateFormOptions) {
  const form = reactive(createTaskCreateFormState());
  const activeInputType = ref<TaskCreateInputType>("url");
  const saveDirectory = useTaskSaveDirectory(form);
  const downloadProxyStore = useDownloadProxyStore();
  const hasProxyStatusError = ref(false);

  const validation = useTaskCreateValidation(form, activeInputType);
  const submission = useTaskCreateSubmission({
    form,
    activeInputType,
    validation,
    rememberSaveDir: saveDirectory.rememberSaveDir,
    resetForm,
    onClose,
    onCreated,
  });
  const canSubmit = computed(
    () =>
      validation.hasValidSourceInput.value &&
      !!form.saveDir &&
      validation.hasValidAdvancedOptions.value &&
      !submission.taskStore.isCreating &&
      !submission.taskStore.isRuntimeExiting &&
      !saveDirectory.isLoadingAccessiblePaths.value,
  );
  const isDialogLocked = computed(() => submission.taskStore.isCreating || submission.taskStore.isRuntimeExiting);
  const isMaskClosable = computed(() => !isDialogLocked.value);
  const isProxyConfigured = computed(
    () => downloadProxyStore.status?.configured === true && !hasProxyStatusError.value,
  );
  const isLoadingProxyStatus = computed(() => downloadProxyStore.isLoading);
  const canUseProxy = computed(() => isProxyConfigured.value && !isLoadingProxyStatus.value);

  watch(
    show,
    (visible) => {
      if (visible) {
        form.useProxy = false;
        submission.clearFeedback();
        void saveDirectory.refreshAccessiblePaths();
        void saveDirectory.detectHostKind();
        void refreshProxyStatus();
      }
    },
    { immediate: true },
  );

  watch(canUseProxy, (available) => {
    if (!available) {
      form.useProxy = false;
    }
  });

  watch(activeInputType, () => {
    submission.clearFeedback();
  });

  function selectTorrentFile(file: File | null) {
    form.torrentFile = file;
  }

  async function refreshProxyStatus() {
    hasProxyStatusError.value = false;
    try {
      await downloadProxyStore.loadStatus();
    } catch {
      hasProxyStatusError.value = true;
      form.useProxy = false;
    }
  }

  function closeDialog() {
    if (isDialogLocked.value) {
      return;
    }

    onClose();
  }

  function openProxySettings() {
    if (isDialogLocked.value) {
      return;
    }

    form.useProxy = false;
    onClose();
    onOpenProxySettings();
  }

  function resetForm() {
    resetTaskCreateFormState(form);
    activeInputType.value = "url";
    submission.clearFeedback();
  }

  return {
    taskStore: submission.taskStore,
    form,
    activeInputType,
    formErrorMessage: submission.formErrorMessage,
    batchFailedItems: submission.batchFailedItems,
    accessiblePaths: saveDirectory.accessiblePaths,
    isLoadingAccessiblePaths: saveDirectory.isLoadingAccessiblePaths,
    accessiblePathsError: saveDirectory.accessiblePathsError,
    hostKind: saveDirectory.hostKind,
    hostSupportsAuthorization: saveDirectory.hostSupportsAuthorization,
    isAuthorizingAccessiblePath: saveDirectory.isAuthorizingAccessiblePath,
    authorizationMessage: saveDirectory.authorizationMessage,
    urlFeedback: validation.urlFeedback,
    urlValidationStatus: validation.urlValidationStatus,
    magnetFeedback: validation.magnetFeedback,
    magnetValidationStatus: validation.magnetValidationStatus,
    accessiblePathOptions: saveDirectory.accessiblePathOptions,
    canSubmit,
    isMaskClosable,
    isProxyConfigured,
    isLoadingProxyStatus,
    hasProxyStatusError,
    canUseProxy,
    selectTorrentFile,
    addAccessiblePath: saveDirectory.addAccessiblePath,
    submitCreateTask: submission.submitCreateTask,
    closeDialog,
    openProxySettings,
  };
}
