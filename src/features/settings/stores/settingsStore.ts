import { defineStore } from "pinia";
import { ref } from "vue";
import { getAccessiblePaths } from "../../../services/storage";
import { getAppConfig, saveAppConfig } from "../../../services/settings";
import { normalizeLanguage, setLanguage, t } from "../../../i18n";
import { getErrorMessage } from "../../../app/utils/errors";
import type { AppConfig } from "../../../types/settings";

export const useSettingsStore = defineStore("settings", () => {
  const config = ref<AppConfig | null>(null);
  const accessiblePaths = ref<string[]>([]);
  const isLoading = ref(false);
  const isLoadingAccessiblePaths = ref(false);
  const isSaving = ref(false);
  const accessiblePathsError = ref("");

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
    try {
      const response = await getAccessiblePaths();
      accessiblePaths.value = response.paths;
      return response.paths;
    } catch (error) {
      accessiblePaths.value = [];
      accessiblePathsError.value = getErrorMessage(error, t("settings.accessiblePathsFailed"));
      throw error;
    } finally {
      isLoadingAccessiblePaths.value = false;
    }
  }

  return {
    config,
    accessiblePaths,
    isLoading,
    isLoadingAccessiblePaths,
    isSaving,
    accessiblePathsError,
    loadConfig,
    loadAccessiblePaths,
    saveConfig,
  };
});

