import { useMessage } from "naive-ui";
import { ref, type Ref } from "vue";
import { getErrorMessage } from "../../../app/utils/errors";
import { useI18n } from "../../../i18n";
import type { CreateBatchDownloadTaskFailure } from "../../../types/tasks";
import { useTaskStore } from "../stores/taskStore";
import {
  buildTaskAdvancedOptions,
  normalizeTaskCategory,
  type TaskCreateFormState,
  type TaskCreateInputType,
} from "./taskCreateFormModel";
import type { useTaskCreateValidation } from "./useTaskCreateValidation";

interface UseTaskCreateSubmissionOptions {
  form: TaskCreateFormState;
  activeInputType: Ref<TaskCreateInputType>;
  validation: ReturnType<typeof useTaskCreateValidation>;
  rememberSaveDir: (path: string) => void;
  resetForm: () => void;
  onClose: () => void;
  onCreated: () => void;
}

export function useTaskCreateSubmission(options: UseTaskCreateSubmissionOptions) {
  const { form, activeInputType, validation, rememberSaveDir, resetForm, onClose, onCreated } = options;
  const taskStore = useTaskStore();
  const message = useMessage();
  const { t } = useI18n();
  const formErrorMessage = ref("");
  const batchFailedItems = ref<CreateBatchDownloadTaskFailure[]>([]);

  async function submitCreateTask() {
    if (taskStore.isRuntimeExiting) {
      message.warning(t("task.runtimeExiting"));
      return;
    }
    const validationMessage = validation.validationError();
    if (validationMessage) {
      formErrorMessage.value = validationMessage;
      return;
    }

    clearFeedback();
    try {
      if (activeInputType.value === "url") {
        await submitUrlTasks();
      } else if (activeInputType.value === "torrent") {
        await submitTorrentTask();
      } else {
        await submitMagnetTask();
      }
    } catch (error) {
      message.error(getErrorMessage(error, t("task.operationFailed")));
    }
  }

  async function submitUrlTasks() {
    const result = await taskStore.createBatchTasks({
      urls: validation.urlList.value,
      saveDir: form.saveDir,
      startMode: form.startMode,
      category: normalizeTaskCategory(form.category),
      advancedOptions: buildTaskAdvancedOptions(form),
    });
    rememberSaveDir(form.saveDir);
    if (result.failed.length > 0) {
      batchFailedItems.value = result.failed;
      formErrorMessage.value = t("create.batch.partialFailed", { count: result.failed.length });
      onCreated();
      return;
    }
    finishSuccessfulCreate();
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

  async function submitMagnetTask() {
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
  }

  function finishSuccessfulCreate() {
    rememberSaveDir(form.saveDir);
    resetForm();
    onClose();
    onCreated();
  }

  function clearFeedback() {
    formErrorMessage.value = "";
    batchFailedItems.value = [];
  }

  return { taskStore, formErrorMessage, batchFailedItems, submitCreateTask, clearFeedback };
}
