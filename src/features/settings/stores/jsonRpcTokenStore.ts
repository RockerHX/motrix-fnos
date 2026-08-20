import { defineStore } from "pinia";
import { ref } from "vue";
import type { JsonRpcTokenStatus } from "../../../types/settings";
import { getJsonRpcTokenStatus, updateJsonRpcToken } from "../services/jsonRpcTokenService";

export const useJsonRpcTokenStore = defineStore("json-rpc-token", () => {
  const status = ref<JsonRpcTokenStatus | null>(null);
  const draftToken = ref("");
  const isLoading = ref(false);
  const isSaving = ref(false);

  async function loadStatus() {
    isLoading.value = true;
    try {
      status.value = await getJsonRpcTokenStatus();
      return status.value;
    } finally {
      isLoading.value = false;
    }
  }

  function generateToken() {
    const bytes = new Uint8Array(32);
    crypto.getRandomValues(bytes);
    draftToken.value = Array.from(bytes, (byte) => byte.toString(16).padStart(2, "0")).join("");
    return draftToken.value;
  }

  async function saveDraft() {
    if (!draftToken.value) return status.value;
    isSaving.value = true;
    try {
      status.value = await updateJsonRpcToken(draftToken.value);
      draftToken.value = "";
      return status.value;
    } finally {
      isSaving.value = false;
    }
  }

  async function clearToken() {
    isSaving.value = true;
    try {
      status.value = await updateJsonRpcToken("");
      draftToken.value = "";
      return status.value;
    } finally {
      isSaving.value = false;
    }
  }

  function clearDraft() {
    draftToken.value = "";
  }

  function clearSensitiveState() {
    status.value = null;
    draftToken.value = "";
    isLoading.value = false;
    isSaving.value = false;
  }

  return { status, draftToken, isLoading, isSaving, loadStatus, generateToken, saveDraft, clearToken, clearDraft, clearSensitiveState };
});
