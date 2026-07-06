import { defineStore } from "pinia";
import { ref } from "vue";
import { clearDebugLogs, listDebugLogs } from "../services/debugLogService";
import { t } from "../../../i18n";
import { getErrorMessage } from "../../../app/utils/errors";
import type { DebugLogEntry } from "../types";

export const useDebugLogStore = defineStore("debugLogs", () => {
  const logs = ref<DebugLogEntry[]>([]);
  const isLoading = ref(false);
  const isClearing = ref(false);
  const errorMessage = ref("");

  async function refreshLogs() {
    isLoading.value = true;
    errorMessage.value = "";

    try {
      logs.value = await listDebugLogs();
    } catch (error) {
      errorMessage.value = getErrorMessage(error, t("logs.failed"));
      throw error;
    } finally {
      isLoading.value = false;
    }
  }

  async function clearLogs() {
    isClearing.value = true;
    errorMessage.value = "";

    try {
      await clearDebugLogs();
      logs.value = [];
    } catch (error) {
      errorMessage.value = getErrorMessage(error, t("logs.failed"));
      throw error;
    } finally {
      isClearing.value = false;
    }
  }

  return {
    logs,
    isLoading,
    isClearing,
    errorMessage,
    refreshLogs,
    clearLogs,
  };
});

