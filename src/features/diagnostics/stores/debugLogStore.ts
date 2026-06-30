import { defineStore } from "pinia";
import { ref } from "vue";
import { clearDebugLogs, listDebugLogs } from "../services/debugLogService";
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
      errorMessage.value = getErrorMessage(error);
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
      errorMessage.value = getErrorMessage(error);
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

function getErrorMessage(error: unknown) {
  if (error instanceof Error) {
    return error.message;
  }

  const message = String(error);
  return message || "调试日志操作失败";
}
