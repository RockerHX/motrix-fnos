import { defineStore } from "pinia";
import { ref } from "vue";
import { getAppConfig, saveAppConfig } from "../../../services/settings";
import type { AppConfig } from "../../../types/settings";
import {
  ensureNotificationPermission,
  getAutoStartEnabled,
  setAutoStartEnabled,
} from "../services/systemIntegrationService";

export const useSettingsStore = defineStore("settings", () => {
  const config = ref<AppConfig | null>(null);
  const isLoading = ref(false);
  const isSaving = ref(false);

  async function loadConfig() {
    isLoading.value = true;
    try {
      const nextConfig = await getAppConfig();
      const autoStartEnabled = await getAutoStartEnabled().catch(() => nextConfig.autoStartEnabled);
      config.value = {
        ...nextConfig,
        autoStartEnabled,
      };
      return config.value;
    } finally {
      isLoading.value = false;
    }
  }

  async function saveConfig(payload: AppConfig) {
    isSaving.value = true;
    try {
      const currentAutoStartEnabled = await getAutoStartEnabled().catch(() => payload.autoStartEnabled);
      if (currentAutoStartEnabled !== payload.autoStartEnabled) {
        await setAutoStartEnabled(payload.autoStartEnabled);
      }
      if (payload.notificationsEnabled) {
        const granted = await ensureNotificationPermission();
        if (!granted) {
          throw new Error("系统通知权限未开启，无法启用下载通知");
        }
      }
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
