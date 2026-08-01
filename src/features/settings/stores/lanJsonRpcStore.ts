import { defineStore } from "pinia";
import { ref } from "vue";
import type { LanJsonRpcStatus } from "../../../types/settings";
import {
  getLanJsonRpcStatus,
  rotateLanJsonRpcToken,
  updateLanJsonRpcEnabled,
} from "../services/lanJsonRpcService";

export const useLanJsonRpcStore = defineStore("lan-json-rpc", () => {
  const status = ref<LanJsonRpcStatus | null>(null);
  const issuedToken = ref("");
  const isLoading = ref(false);
  const isSaving = ref(false);
  let sensitiveGeneration = 0;

  async function loadStatus() {
    isLoading.value = true;
    try {
      status.value = await getLanJsonRpcStatus();
      return status.value;
    } finally {
      isLoading.value = false;
    }
  }

  async function setEnabled(enabled: boolean) {
    const previousStatus = status.value;
    const requestGeneration = sensitiveGeneration;
    isSaving.value = true;
    issuedToken.value = "";
    try {
      const response = await updateLanJsonRpcEnabled(enabled);
      status.value = response.status;
      issuedToken.value = requestGeneration === sensitiveGeneration ? (response.issuedToken ?? "") : "";
      return response;
    } catch (error) {
      status.value = previousStatus ? { ...previousStatus } : null;
      throw error;
    } finally {
      isSaving.value = false;
    }
  }

  async function rotateToken() {
    const requestGeneration = sensitiveGeneration;
    isSaving.value = true;
    issuedToken.value = "";
    try {
      const response = await rotateLanJsonRpcToken();
      status.value = response.status;
      issuedToken.value = requestGeneration === sensitiveGeneration ? (response.issuedToken ?? "") : "";
      return response;
    } finally {
      isSaving.value = false;
    }
  }

  function clearIssuedToken() {
    issuedToken.value = "";
  }

  function clearSensitiveState() {
    sensitiveGeneration += 1;
    issuedToken.value = "";
  }

  return {
    status,
    issuedToken,
    isLoading,
    isSaving,
    loadStatus,
    setEnabled,
    rotateToken,
    clearIssuedToken,
    clearSensitiveState,
  };
});
