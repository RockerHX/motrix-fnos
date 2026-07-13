import { computed, onMounted, reactive, ref, watch, type Ref } from "vue";
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
}

export function useTaskCreateForm({ show, onClose, onCreated }: UseTaskCreateFormOptions) {
  const form = reactive(createTaskCreateFormState());
  const activeInputType = ref<TaskCreateInputType>("url");
  const saveDirectory = useTaskSaveDirectory(form);

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

  watch(show, (visible) => {
    if (visible) {
      submission.clearFeedback();
      void saveDirectory.refreshAccessiblePaths();
    }
  });

  watch(activeInputType, () => {
    submission.clearFeedback();
  });

  onMounted(() => {
    void saveDirectory.refreshAccessiblePaths();
  });

  function selectTorrentFile(file: File | null) {
    form.torrentFile = file;
  }

  function closeDialog() {
    if (isDialogLocked.value) {
      return;
    }

    onClose();
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
    urlFeedback: validation.urlFeedback,
    urlValidationStatus: validation.urlValidationStatus,
    magnetFeedback: validation.magnetFeedback,
    magnetValidationStatus: validation.magnetValidationStatus,
    accessiblePathOptions: saveDirectory.accessiblePathOptions,
    canSubmit,
    isMaskClosable,
    selectTorrentFile,
    submitCreateTask: submission.submitCreateTask,
    closeDialog,
  };
}
