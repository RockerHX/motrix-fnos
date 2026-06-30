import { defineStore } from "pinia";
import { ref } from "vue";
import { getAppConfig, saveAppConfig } from "../../../services/settings";
import type { AppConfig } from "../../../types/settings";

export const useSettingsStore = defineStore("settings", () => {
  const config = ref<AppConfig | null>(null);
  const isLoading = ref(false);
  const isSaving = ref(false);

  async function loadConfig() {
    isLoading.value = true;
    try {
      config.value = await getAppConfig();
      return config.value;
    } finally {
      isLoading.value = false;
    }
  }

  async function saveConfig(payload: AppConfig) {
    isSaving.value = true;
    try {
      config.value = await saveAppConfig(payload);
      return config.value;
    } finally {
      isSaving.value = false;
    }
  }

  return {
    config,
    isLoading,
    isSaving,
    loadConfig,
    saveConfig,
  };
});
