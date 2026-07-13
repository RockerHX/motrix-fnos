import { computed, onMounted, reactive, ref, watch, type Ref } from "vue";
import { useMessage } from "naive-ui";
import { useI18n } from "../../../i18n";
import { getErrorMessage } from "../../../app/utils/errors";
import { useTaskStore } from "../stores/taskStore";
import type {
  CreateBatchDownloadTaskFailure,
} from "../../../types/tasks";
import {
  buildTaskAdvancedOptions,
  createTaskCreateFormState,
  normalizeTaskCategory,
  resetTaskCreateFormState,
  type TaskCreateInputType,
} from "./taskCreateFormModel";
import { useTaskCreateValidation } from "./useTaskCreateValidation";
import { useTaskSaveDirectory } from "./useTaskSaveDirectory";

interface UseTaskCreateFormOptions {
  show: Ref<boolean>;
  onClose: () => void;
  onCreated: () => void;
}

export function useTaskCreateForm({ show, onClose, onCreated }: UseTaskCreateFormOptions) {
  const taskStore = useTaskStore();
  const message = useMessage();
  const { t } = useI18n();

  const form = reactive(createTaskCreateFormState());
  const activeInputType = ref<TaskCreateInputType>("url");
  const formErrorMessage = ref("");
  const batchFailedItems = ref<CreateBatchDownloadTaskFailure[]>([]);
  const saveDirectory = useTaskSaveDirectory(form);

  const validation = useTaskCreateValidation(form, activeInputType);
  const canSubmit = computed(
    () =>
      validation.hasValidSourceInput.value &&
      !!form.saveDir &&
      validation.hasValidAdvancedOptions.value &&
      !taskStore.isCreating &&
      !taskStore.isRuntimeExiting &&
      !saveDirectory.isLoadingAccessiblePaths.value,
  );
  const isDialogLocked = computed(() => taskStore.isCreating || taskStore.isRuntimeExiting);
  const isMaskClosable = computed(() => !isDialogLocked.value);

  watch(show, (visible) => {
    if (visible) {
      formErrorMessage.value = "";
      batchFailedItems.value = [];
      void saveDirectory.refreshAccessiblePaths();
    }
  });

  watch(activeInputType, () => {
    formErrorMessage.value = "";
    batchFailedItems.value = [];
  });

  onMounted(() => {
    void saveDirectory.refreshAccessiblePaths();
  });

  async function submitCreateTask() {
    if (taskStore.isRuntimeExiting) {
      message.warning(t("task.runtimeExiting"));
      return;
    }
    const validationError = validation.validationError();
    if (validationError) {
      formErrorMessage.value = validationError;
      return;
    }

    formErrorMessage.value = "";
    batchFailedItems.value = [];

    try {
      if (activeInputType.value === "url") {
        await submitUrlTasks();
        return;
      }

      if (activeInputType.value === "torrent") {
        await submitTorrentTask();
        return;
      }

      await taskStore.createTask({
        url: form.magnet.trim(),
        fileName: null,
        saveDir: form.saveDir,
        sourceType: "magnet",
        startMode: form.startMode,
        category: normalizeTaskCategory(form.category),
        advancedOptions: buildTaskAdvancedOptions(form),
      });
      finishSuccessfulCreate();
    } catch (error) {
      message.error(getErrorMessage(error, t("task.operationFailed")));
    }
  }

  async function submitUrlTasks() {
    try {
      const result = await taskStore.createBatchTasks({
        urls: validation.urlList.value,
        saveDir: form.saveDir,
        startMode: form.startMode,
        category: normalizeTaskCategory(form.category),
        advancedOptions: buildTaskAdvancedOptions(form),
      });
      saveDirectory.rememberSaveDir(form.saveDir);
      if (result.failed.length > 0) {
        batchFailedItems.value = result.failed;
        formErrorMessage.value = t("create.batch.partialFailed", { count: result.failed.length });
        onCreated();
        return;
      }
      resetForm();
      onClose();
      onCreated();
    } catch (error) {
      message.error(getErrorMessage(error, t("task.operationFailed")));
    }
  }

  async function submitTorrentTask() {
    if (!form.torrentFile) {
      formErrorMessage.value = t("create.torrent.required");
      return;
    }

    await taskStore.createTorrentTask({
      torrent: form.torrentFile,
      saveDir: form.saveDir,
      startMode: form.startMode,
      category: normalizeTaskCategory(form.category),
      advancedOptions: buildTaskAdvancedOptions(form),
    });
    finishSuccessfulCreate();
  }

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
    formErrorMessage.value = "";
    batchFailedItems.value = [];
  }

  function finishSuccessfulCreate() {
    saveDirectory.rememberSaveDir(form.saveDir);
    resetForm();
    onClose();
    onCreated();
  }

  return {
    taskStore,
    form,
    activeInputType,
    formErrorMessage,
    batchFailedItems,
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
    submitCreateTask,
    closeDialog,
  };
}
