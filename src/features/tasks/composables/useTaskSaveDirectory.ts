import { computed, ref } from "vue";
import { getErrorMessage } from "../../../app/utils/errors";
import { useI18n } from "../../../i18n";
import { fnosHost, type FnosHostKind } from "../../../services/fnos";
import { getAccessiblePaths, refreshAccessiblePaths as refreshAccessiblePathsFromApi } from "../../../services/storage";
import { useSettingsStore } from "../../settings/stores/settingsStore";
import type { TaskCreateFormState } from "./taskCreateFormModel";

const LAST_SAVE_DIR_KEY = "motrix-fnos:last-save-dir";

export function useTaskSaveDirectory(form: TaskCreateFormState) {
  const settingsStore = useSettingsStore();
  const { t } = useI18n();
  const accessiblePaths = ref<string[]>([]);
  const isLoadingAccessiblePaths = ref(false);
  const accessiblePathsError = ref("");
  const hostKind = ref<FnosHostKind | null>(null);
  const isAuthorizingAccessiblePath = ref(false);
  const authorizationMessage = ref("");
  const hostSupportsAuthorization = computed(() => hostKind.value === "hosted" || hostKind.value === "mobile");
  const accessiblePathOptions = computed(() =>
    accessiblePaths.value.map((path) => ({ label: path, value: path })),
  );

  async function refreshAccessiblePaths(options: { queryOfficial?: boolean } = {}) {
    isLoadingAccessiblePaths.value = true;
    accessiblePathsError.value = "";
    authorizationMessage.value = "";

    try {
      const pathsRequest = options.queryOfficial ? refreshAccessiblePathsFromApi() : getAccessiblePaths();
      const [response, config] = await Promise.all([pathsRequest, settingsStore.loadConfig()]);
      accessiblePaths.value = response.paths;
      syncSelectedSaveDir(config.defaultDownloadDir);
      return true;
    } catch (error) {
      accessiblePaths.value = [];
      form.saveDir = "";
      accessiblePathsError.value = getErrorMessage(error, t("task.operationFailed"));
      return false;
    } finally {
      isLoadingAccessiblePaths.value = false;
    }
  }

  async function detectHostKind() {
    hostKind.value = await fnosHost.getHostKind();
    return hostKind.value;
  }

  async function addAccessiblePath() {
    if (isAuthorizingAccessiblePath.value || !hostSupportsAuthorization.value) {
      authorizationMessage.value = t("settings.accessiblePaths.manualHelp");
      return;
    }

    isAuthorizingAccessiblePath.value = true;
    authorizationMessage.value = "";
    try {
      const result = await fnosHost.requestSharedFolderAuthorization();
      if (result.status === "cancelled") return;
      if (result.status === "admin_required") {
        authorizationMessage.value = t("settings.accessiblePaths.adminRequired");
        return;
      }
      if (result.status === "unsupported") {
        authorizationMessage.value = t("settings.accessiblePaths.manualHelp");
        return;
      }
      if (result.status === "failed") {
        authorizationMessage.value = t("settings.accessiblePaths.failed");
        return;
      }

      const refreshed = await refreshAccessiblePaths({ queryOfficial: true });
      if (!refreshed) {
        authorizationMessage.value = t("settings.accessiblePaths.stale");
      }
    } finally {
      isAuthorizingAccessiblePath.value = false;
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
    hostKind,
    hostSupportsAuthorization,
    isAuthorizingAccessiblePath,
    authorizationMessage,
    accessiblePathOptions,
    detectHostKind,
    refreshAccessiblePaths,
    addAccessiblePath,
    rememberSaveDir,
  };
}
