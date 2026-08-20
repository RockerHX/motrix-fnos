import { defineStore } from "pinia";
import { ref } from "vue";
import type { DownloadProxyMutationResponse, DownloadProxyStatus } from "../../../types/settings";
import {
  deleteDownloadProxy,
  getDownloadProxyStatus,
  updateDownloadProxy,
} from "../services/downloadProxyService";

export const useDownloadProxyStore = defineStore("download-proxy", () => {
  const status = ref<DownloadProxyStatus | null>(null);
  const draftProxyUrl = ref("");
  const lastMutation = ref<DownloadProxyMutationResponse | null>(null);
  const isLoading = ref(false);
  const isSaving = ref(false);
  let transientGeneration = 0;

  async function loadStatus() {
    isLoading.value = true;
    try {
      status.value = await getDownloadProxyStatus();
      return status.value;
    } catch (error) {
      status.value = null;
      throw error;
    } finally {
      isLoading.value = false;
    }
  }

  function setDraftProxyUrl(value: string) {
    draftProxyUrl.value = value;
  }

  async function saveDraft() {
    const proxyUrl = draftProxyUrl.value;
    if (!proxyUrl.trim()) return null;
    const requestGeneration = transientGeneration;
    draftProxyUrl.value = "";
    lastMutation.value = null;
    isSaving.value = true;
    try {
      const response = await updateDownloadProxy(proxyUrl);
      status.value = response.status;
      if (requestGeneration === transientGeneration) {
        lastMutation.value = response;
      }
      return response;
    } finally {
      isSaving.value = false;
    }
  }

  async function clearProxy() {
    draftProxyUrl.value = "";
    lastMutation.value = null;
    isSaving.value = true;
    try {
      await deleteDownloadProxy();
      status.value = { configured: false, maskedProxyUrl: null, revision: 0 };
      return status.value;
    } finally {
      isSaving.value = false;
    }
  }

  function clearTransientState() {
    transientGeneration += 1;
    draftProxyUrl.value = "";
    lastMutation.value = null;
  }

  return {
    status,
    draftProxyUrl,
    lastMutation,
    isLoading,
    isSaving,
    loadStatus,
    setDraftProxyUrl,
    saveDraft,
    clearProxy,
    clearTransientState,
  };
});
