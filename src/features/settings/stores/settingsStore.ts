import { defineStore } from "pinia";
import { ref } from "vue";
import {
  getAccessiblePaths,
  getDisplayAccessiblePaths,
  refreshAccessiblePaths as refreshAccessiblePathsFromApi,
} from "../../../services/storage";
import { getAppConfig, saveAppConfig } from "../../../services/settings";
import { language, normalizeLanguage, setLanguage, t, type AppLanguage } from "../../../i18n";
import { getErrorMessage } from "../../../app/utils/errors";
import type { AppConfig } from "../../../types/settings";
import type { DisplayPath } from "../../../types/storage";

export const useSettingsStore = defineStore("settings", () => {
  const config = ref<AppConfig | null>(null);
  const accessiblePaths = ref<string[]>([]);
  const displayAccessiblePaths = ref<DisplayPath[]>([]);
  const isLoading = ref(false);
  const isLoadingAccessiblePaths = ref(false);
  const isSaving = ref(false);
  const accessiblePathsError = ref("");
  const accessiblePathsStale = ref(false);

  async function loadConfig() {
    isLoading.value = true;
    try {
      config.value = await getAppConfig();
      config.value.language = normalizeLanguage(config.value.language);
      setLanguage(config.value.language);
      return config.value;
    } finally {
      isLoading.value = false;
    }
  }

  async function saveConfig(payload: AppConfig) {
    isSaving.value = true;
    try {
      config.value = await saveAppConfig({
        ...payload,
        language: normalizeLanguage(payload.language),
      });
      config.value.language = normalizeLanguage(config.value.language);
      setLanguage(config.value.language);
      return config.value;
    } finally {
      isSaving.value = false;
    }
  }

  async function loadAccessiblePaths() {
    isLoadingAccessiblePaths.value = true;
    accessiblePathsError.value = "";
    accessiblePathsStale.value = false;
    try {
      const response = await getAccessiblePaths();
      accessiblePaths.value = response.paths;
      await loadDisplayAccessiblePaths(language.value);
      return response.paths;
    } catch (error) {
      accessiblePaths.value = [];
      displayAccessiblePaths.value = [];
      accessiblePathsError.value = getErrorMessage(error, t("settings.accessiblePathsFailed"));
      throw error;
    } finally {
      isLoadingAccessiblePaths.value = false;
    }
  }

  async function refreshAccessiblePaths() {
    isLoadingAccessiblePaths.value = true;
    accessiblePathsError.value = "";
    accessiblePathsStale.value = false;
    try {
      const response = await refreshAccessiblePathsFromApi();
      accessiblePaths.value = response.paths;
      await loadDisplayAccessiblePaths(language.value);
      return response.paths;
    } catch (error) {
      accessiblePathsStale.value = true;
      accessiblePathsError.value = getErrorMessage(error, t("settings.accessiblePaths.refreshFailed"));
      try {
        const response = await getAccessiblePaths();
        accessiblePaths.value = response.paths;
        await loadDisplayAccessiblePaths(language.value);
      } catch {
        // Keep the last in-memory snapshot when the fallback read also fails.
      }
      throw error;
    } finally {
      isLoadingAccessiblePaths.value = false;
    }
  }

  async function loadDisplayAccessiblePaths(nextLanguage: AppLanguage = language.value) {
    try {
      const response = await getDisplayAccessiblePaths(nextLanguage);
      displayAccessiblePaths.value = alignDisplayPaths(accessiblePaths.value, response.paths);
    } catch {
      displayAccessiblePaths.value = fallbackDisplayPaths(accessiblePaths.value);
    }
    return displayAccessiblePaths.value;
  }

  function clearSensitiveState() {
    config.value = null;
    accessiblePaths.value = [];
    displayAccessiblePaths.value = [];
    isLoading.value = false;
    isLoadingAccessiblePaths.value = false;
    isSaving.value = false;
    accessiblePathsError.value = "";
    accessiblePathsStale.value = false;
  }

  return {
    config,
    accessiblePaths,
    displayAccessiblePaths,
    isLoading,
    isLoadingAccessiblePaths,
    isSaving,
    accessiblePathsError,
    accessiblePathsStale,
    loadConfig,
    loadAccessiblePaths,
    refreshAccessiblePaths,
    loadDisplayAccessiblePaths,
    saveConfig,
    clearSensitiveState,
  };
});

function alignDisplayPaths(paths: string[], displayPaths: DisplayPath[]) {
  return paths.map((path) => {
    const matches = displayPaths.filter((item) => item.path === path);
    const displayPath = matches.length === 1 && matches[0].displayPath.trim() ? matches[0].displayPath : path;
    return { path, displayPath };
  });
}

function fallbackDisplayPaths(paths: string[]) {
  return paths.map((path) => ({ path, displayPath: path }));
}
