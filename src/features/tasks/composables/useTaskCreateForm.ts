import { computed, onMounted, reactive, ref, watch, type Ref } from "vue";
import { useMessage } from "naive-ui";
import { getAccessiblePaths } from "../../../services/storage";
import { useI18n } from "../../../i18n";
import { getErrorMessage } from "../../../app/utils/errors";
import { useSettingsStore } from "../../settings/stores/settingsStore";
import { useTaskStore } from "../stores/taskStore";
import type {
  CreateBatchDownloadTaskFailure,
  CreateTaskAdvancedOptions,
  DownloadTaskStartMode,
} from "../../../types/tasks";

const LAST_SAVE_DIR_KEY = "motrix-fnos:last-save-dir";
const DEFAULT_CATEGORY = "默认";

type TaskCreateInputType = "url" | "batch" | "torrent" | "magnet";

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
    batchUrls: "",
    magnet: "",
    torrentFile: null as File | null,
    fileName: "",
    saveDir: "",
    startMode: "now" as DownloadTaskStartMode,
    category: DEFAULT_CATEGORY,
    connections: 16,
    downloadLimitKb: 0,
    proxy: "",
  });
  const activeInputType = ref<TaskCreateInputType>("url");
  const formErrorMessage = ref("");
  const batchFailedItems = ref<CreateBatchDownloadTaskFailure[]>([]);
  const accessiblePaths = ref<string[]>([]);
  const isLoadingAccessiblePaths = ref(false);
  const accessiblePathsError = ref("");

  const isUrlValid = computed(() => /^https?:\/\/.+/i.test(form.url.trim()));
  const isMagnetValid = computed(() => /^magnet:\?/i.test(form.magnet.trim()));
  const batchUrlList = computed(() =>
    form.batchUrls
      .split(/\r?\n/)
      .map((url) => url.trim())
      .filter(Boolean),
  );
  const urlFeedback = computed(() => (form.url && !isUrlValid.value ? t("create.url.invalid") : undefined));
  const urlValidationStatus = computed(() => (form.url && !isUrlValid.value ? "error" : undefined));
  const magnetFeedback = computed(() =>
    form.magnet && !isMagnetValid.value ? t("create.magnet.invalid") : undefined,
  );
  const magnetValidationStatus = computed(() => (form.magnet && !isMagnetValid.value ? "error" : undefined));
  const accessiblePathOptions = computed(() =>
    accessiblePaths.value.map((path) => ({
      label: path,
      value: path,
    })),
  );
  const canSubmit = computed(
    () =>
      hasValidSourceInput() &&
      !!form.saveDir &&
      hasValidAdvancedOptions() &&
      !taskStore.isCreating &&
      !taskStore.isRuntimeExiting &&
      !isLoadingAccessiblePaths.value,
  );
  const isDialogLocked = computed(() => taskStore.isCreating || taskStore.isRuntimeExiting);
  const isMaskClosable = computed(() => !isDialogLocked.value);

  watch(show, (visible) => {
    if (visible) {
      formErrorMessage.value = "";
      batchFailedItems.value = [];
      void refreshAccessiblePaths();
    }
  });

  watch(activeInputType, () => {
    formErrorMessage.value = "";
    batchFailedItems.value = [];
  });

  onMounted(() => {
    void refreshAccessiblePaths();
  });

  async function submitCreateTask() {
    if (taskStore.isRuntimeExiting) {
      message.warning(t("task.runtimeExiting"));
      return;
    }
    if (!validateForm()) {
      return;
    }

    formErrorMessage.value = "";
    batchFailedItems.value = [];

    try {
      if (activeInputType.value === "batch") {
        await submitBatchTasks();
        return;
      }

      if (activeInputType.value === "torrent") {
        await submitTorrentTask();
        return;
      }

      await taskStore.createTask({
        url: activeInputType.value === "magnet" ? form.magnet.trim() : form.url.trim(),
        fileName: activeInputType.value === "url" ? optionalText(form.fileName) : null,
        saveDir: form.saveDir,
        sourceType: activeInputType.value === "magnet" ? "magnet" : "url",
        startMode: form.startMode,
        category: normalizedCategory(),
        advancedOptions: buildAdvancedOptions(),
      });
      finishSuccessfulCreate();
    } catch (error) {
      message.error(getErrorMessage(error, t("task.operationFailed")));
    }
  }

  async function submitBatchTasks() {
    try {
      const result = await taskStore.createBatchTasks({
        urls: batchUrlList.value,
        saveDir: form.saveDir,
        startMode: form.startMode,
        category: normalizedCategory(),
        advancedOptions: buildAdvancedOptions(),
      });
      rememberSaveDir(form.saveDir);
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
      category: normalizedCategory(),
      advancedOptions: buildAdvancedOptions(),
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
    form.url = "";
    form.batchUrls = "";
    form.magnet = "";
    form.torrentFile = null;
    form.fileName = "";
    form.saveDir = "";
    form.startMode = "now";
    form.category = DEFAULT_CATEGORY;
    form.connections = 16;
    form.downloadLimitKb = 0;
    form.proxy = "";
    activeInputType.value = "url";
    formErrorMessage.value = "";
    batchFailedItems.value = [];
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
      accessiblePathsError.value = getErrorMessage(error, t("task.operationFailed"));
    } finally {
      isLoadingAccessiblePaths.value = false;
    }
  }

  function validateForm() {
    if (activeInputType.value === "url" && !isUrlValid.value) {
      formErrorMessage.value = t("create.url.required");
      return false;
    }
    if (activeInputType.value === "batch" && batchUrlList.value.length === 0) {
      formErrorMessage.value = t("create.batch.required");
      return false;
    }
    if (activeInputType.value === "torrent" && !form.torrentFile) {
      formErrorMessage.value = t("create.torrent.required");
      return false;
    }
    if (activeInputType.value === "magnet" && !isMagnetValid.value) {
      formErrorMessage.value = t("create.magnet.required");
      return false;
    }
    if (!form.saveDir) {
      formErrorMessage.value = t("create.saveDir.required");
      return false;
    }
    if (!hasValidAdvancedOptions()) {
      formErrorMessage.value = t("create.advanced.invalid");
      return false;
    }
    return true;
  }

  function hasValidSourceInput() {
    if (activeInputType.value === "url") {
      return isUrlValid.value;
    }
    if (activeInputType.value === "batch") {
      return batchUrlList.value.length > 0;
    }
    if (activeInputType.value === "torrent") {
      return !!form.torrentFile;
    }
    return isMagnetValid.value;
  }

  function hasValidAdvancedOptions() {
    return form.connections >= 1 && form.connections <= 64 && form.downloadLimitKb >= 0;
  }

  function buildAdvancedOptions(): CreateTaskAdvancedOptions {
    return {
      connections: form.connections,
      downloadLimitKb: form.downloadLimitKb,
      proxy: optionalText(form.proxy),
    };
  }

  function normalizedCategory() {
    return optionalText(form.category) || DEFAULT_CATEGORY;
  }

  function optionalText(value: string) {
    const trimmed = value.trim();
    return trimmed ? trimmed : null;
  }

  function finishSuccessfulCreate() {
    rememberSaveDir(form.saveDir);
    resetForm();
    onClose();
    onCreated();
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

  return {
    taskStore,
    form,
    activeInputType,
    formErrorMessage,
    batchFailedItems,
    accessiblePaths,
    isLoadingAccessiblePaths,
    accessiblePathsError,
    urlFeedback,
    urlValidationStatus,
    magnetFeedback,
    magnetValidationStatus,
    accessiblePathOptions,
    canSubmit,
    isMaskClosable,
    selectTorrentFile,
    submitCreateTask,
    closeDialog,
  };
}
