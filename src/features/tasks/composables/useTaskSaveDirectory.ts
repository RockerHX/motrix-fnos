import { computed, ref } from "vue";
import { getErrorMessage } from "../../../app/utils/errors";
import { useI18n } from "../../../i18n";
import { getAccessiblePaths } from "../../../services/storage";
import { useSettingsStore } from "../../settings/stores/settingsStore";
import type { TaskCreateFormState } from "./taskCreateFormModel";

const LAST_SAVE_DIR_KEY = "motrix-fnos:last-save-dir";

export function useTaskSaveDirectory(form: TaskCreateFormState) {
  const settingsStore = useSettingsStore();
  const { t } = useI18n();
  const accessiblePaths = ref<string[]>([]);
  const isLoadingAccessiblePaths = ref(false);
  const accessiblePathsError = ref("");
  const accessiblePathOptions = computed(() =>
    accessiblePaths.value.map((path) => ({ label: path, value: path })),
  );

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

  function syncSelectedSaveDir(defaultDownloadDir: string) {
    if (form.saveDir && accessiblePaths.value.includes(form.saveDir)) return;

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
    if (typeof localStorage !== "undefined") localStorage.setItem(LAST_SAVE_DIR_KEY, path);
  }

  function readRememberedSaveDir() {
    if (typeof localStorage === "undefined") return "";
    return localStorage.getItem(LAST_SAVE_DIR_KEY) || "";
  }

  return {
    accessiblePaths,
    isLoadingAccessiblePaths,
    accessiblePathsError,
    accessiblePathOptions,
    refreshAccessiblePaths,
    rememberSaveDir,
  };
}
