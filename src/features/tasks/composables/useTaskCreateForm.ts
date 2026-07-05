import { computed, onMounted, reactive, ref, watch, type Ref } from "vue";
import { useMessage } from "naive-ui";
import { getAccessiblePaths } from "../../../services/storage";
import { useI18n } from "../../../i18n";
import { useSettingsStore } from "../../settings/stores/settingsStore";
import { useTaskStore } from "../stores/taskStore";

const LAST_SAVE_DIR_KEY = "motrix-fnos:last-save-dir";

interface UseTaskCreateFormOptions {
  show: Ref<boolean>;
  onClose: () => void;
  onCreated: () => void;
}

export function useTaskCreateForm({ show, onClose, onCreated }: UseTaskCreateFormOptions) {
  const taskStore = useTaskStore();
  const settingsStore = useSettingsStore();
  const message = useMessage();
  const { t } = useI18n();

  const form = reactive({
    url: "",
    fileName: "",
    saveDir: "",
    startMode: "now",
    note: "",
  });
  const activeInputType = ref("url");
  const formErrorMessage = ref("");
  const accessiblePaths = ref<string[]>([]);
  const isLoadingAccessiblePaths = ref(false);
  const accessiblePathsError = ref("");

  const isUrlValid = computed(() => /^https?:\/\/.+/i.test(form.url.trim()));
  const urlFeedback = computed(() => (form.url && !isUrlValid.value ? t("create.url.invalid") : undefined));
  const urlValidationStatus = computed(() => (form.url && !isUrlValid.value ? "error" : undefined));
  const accessiblePathOptions = computed(() =>
    accessiblePaths.value.map((path) => ({
      label: path,
      value: path,
    })),
  );
  const canSubmit = computed(
    () =>
      isUrlValid.value &&
      !!form.saveDir &&
      !taskStore.isCreating &&
      !taskStore.isRuntimeExiting &&
      !isLoadingAccessiblePaths.value,
  );
  const isDialogLocked = computed(() => taskStore.isCreating || taskStore.isRuntimeExiting);
  const isMaskClosable = computed(() => !isDialogLocked.value);

  watch(show, (visible) => {
    if (visible) {
      formErrorMessage.value = "";
      void refreshAccessiblePaths();
    }
  });

  onMounted(() => {
    void refreshAccessiblePaths();
  });

  async function submitCreateTask() {
    if (taskStore.isRuntimeExiting) {
      message.warning(t("task.runtimeExiting"));
      return;
    }
    if (!isUrlValid.value) {
      formErrorMessage.value = t("create.url.required");
      return;
    }
    if (!form.saveDir) {
      formErrorMessage.value = t("create.saveDir.required");
      return;
    }

    formErrorMessage.value = "";

    try {
      await taskStore.createTask({
        url: form.url,
        fileName: form.fileName || null,
        saveDir: form.saveDir,
      });
      rememberSaveDir(form.saveDir);
      resetForm();
      onClose();
      onCreated();
    } catch (error) {
      message.error(getErrorMessage(error));
    }
  }

  function closeDialog() {
    if (isDialogLocked.value) {
      return;
    }

    onClose();
  }

  function resetForm() {
    form.url = "";
    form.fileName = "";
    form.saveDir = "";
    form.startMode = "now";
    form.note = "";
    activeInputType.value = "url";
    formErrorMessage.value = "";
  }

  async function refreshAccessiblePaths() {
    isLoadingAccessiblePaths.value = true;
    accessiblePathsError.value = "";

    try {
      const [response, config] = await Promise.all([getAccessiblePaths(), settingsStore.loadConfig()]);
      accessiblePaths.value = response.paths;
      syncSelectedSaveDir(config.defaultDownloadDir);
    } catch (error) {
      accessiblePaths.value = [];
      form.saveDir = "";
      accessiblePathsError.value = getErrorMessage(error);
    } finally {
      isLoadingAccessiblePaths.value = false;
    }
  }

  function syncSelectedSaveDir(defaultDownloadDir: string) {
    if (form.saveDir && accessiblePaths.value.includes(form.saveDir)) {
      return;
    }

    const remembered = readRememberedSaveDir();
    if (defaultDownloadDir && accessiblePaths.value.includes(defaultDownloadDir)) {
      form.saveDir = defaultDownloadDir;
      return;
    }
    if (remembered && accessiblePaths.value.includes(remembered)) {
      form.saveDir = remembered;
      return;
    }
    form.saveDir = accessiblePaths.value[0] || "";
  }

  function rememberSaveDir(path: string) {
    if (typeof localStorage === "undefined") {
      return;
    }

    localStorage.setItem(LAST_SAVE_DIR_KEY, path);
  }

  function readRememberedSaveDir() {
    if (typeof localStorage === "undefined") {
      return "";
    }

    return localStorage.getItem(LAST_SAVE_DIR_KEY) || "";
  }

  function getErrorMessage(error: unknown) {
    if (error instanceof Error) {
      return error.message;
    }

    const message = String(error);
    return message || t("task.operationFailed");
  }

  return {
    taskStore,
    form,
    activeInputType,
    formErrorMessage,
    accessiblePaths,
    isLoadingAccessiblePaths,
    accessiblePathsError,
    urlFeedback,
    urlValidationStatus,
    accessiblePathOptions,
    canSubmit,
    isMaskClosable,
    submitCreateTask,
    closeDialog,
  };
}
