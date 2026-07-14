import { defineStore } from "pinia";
import { computed, ref } from "vue";
import { useDebugLogStore } from "../../diagnostics/stores/debugLogStore";
import { useSettingsStore } from "../../settings/stores/settingsStore";
import { useTaskStore } from "../../tasks/stores/taskStore";
import { setCsrfTokenProvider, setUnauthorizedHandler } from "../../../services/http";
import { createAuthChannel, type AuthChannel } from "../services/authChannel";
import {
  changeAuthPassword,
  changeAuthProtection,
  getAuthStatus,
  loginAuth,
  logoutAuth,
  setupAuth,
} from "../services/authService";
import type { AuthChannelMessage, AuthPhase, AuthStatus, ChangePasswordRequest } from "../types";

export const useAuthStore = defineStore("auth", () => {
  const phase = ref<AuthPhase>("loading");
  const enabled = ref(true);
  const authenticated = ref(false);
  const csrfToken = ref<string | null>(null);
  const errorMessage = ref("");
  const isSubmitting = ref(false);
  let channel: AuthChannel | null = null;

  const isReady = computed(() => phase.value === "ready");

  async function initialize() {
    phase.value = "loading";
    errorMessage.value = "";
    try {
      applyStatus(await getAuthStatus());
    } catch (error) {
      clearAuthState();
      phase.value = "error";
      errorMessage.value = error instanceof Error ? error.message : String(error);
      throw error;
    }
  }

  async function refreshStatus() {
    const status = await getAuthStatus();
    applyStatus(status);
    return status;
  }

  async function setup(password: string) {
    return submit(() => setupAuth(password));
  }

  async function login(password: string) {
    return submit(() => loginAuth(password));
  }

  async function logout() {
    isSubmitting.value = true;
    try {
      await logoutAuth();
    } finally {
      lockToLogin();
      channel?.post({ type: "session-invalidated" });
      isSubmitting.value = false;
    }
  }

  async function changePassword(payload: ChangePasswordRequest) {
    const status = await changeAuthPassword(payload);
    applyStatus(status);
    channel?.post({ type: "auth-updated" });
    return status;
  }

  async function setProtection(nextEnabled: boolean, currentPassword: string) {
    const status = await changeAuthProtection({ enabled: nextEnabled, currentPassword });
    applyStatus(status);
    channel?.post({ type: "auth-updated" });
    return status;
  }

  async function handleUnauthorized() {
    clearSensitiveState();
    clearAuthState();
    try {
      applyStatus(await getAuthStatus());
    } catch {
      phase.value = "login";
    }
    channel?.post({ type: "session-invalidated" });
  }

  function startCoordination() {
    setCsrfTokenProvider(() => csrfToken.value);
    setUnauthorizedHandler(handleUnauthorized);
    channel ??= createAuthChannel(handleChannelMessage);
  }

  function stopCoordination() {
    setCsrfTokenProvider(null);
    setUnauthorizedHandler(null);
    channel?.close();
    channel = null;
  }

  async function submit(operation: () => Promise<AuthStatus>) {
    isSubmitting.value = true;
    errorMessage.value = "";
    try {
      const status = await operation();
      applyStatus(status);
      channel?.post({ type: "auth-updated" });
      return status;
    } finally {
      isSubmitting.value = false;
    }
  }

  async function handleChannelMessage(message: AuthChannelMessage) {
    if (message.type === "session-invalidated") {
      lockToLogin();
      return;
    }
    try {
      await refreshStatus();
    } catch {
      lockToLogin();
    }
  }

  function applyStatus(status: AuthStatus) {
    enabled.value = status.enabled;
    authenticated.value = status.authenticated;
    csrfToken.value = status.csrfToken;
    errorMessage.value = "";
    if (status.setupRequired) {
      phase.value = "setup";
      csrfToken.value = null;
    } else if (status.enabled && !status.authenticated) {
      phase.value = "login";
      csrfToken.value = null;
    } else {
      phase.value = "ready";
    }
  }

  function lockToLogin() {
    clearSensitiveState();
    clearAuthState();
    phase.value = "login";
  }

  function clearAuthState() {
    authenticated.value = false;
    csrfToken.value = null;
  }

  function clearSensitiveState() {
    useTaskStore().clearSensitiveState();
    useSettingsStore().clearSensitiveState();
    useDebugLogStore().clearSensitiveState();
  }

  return {
    phase,
    enabled,
    authenticated,
    csrfToken,
    errorMessage,
    isSubmitting,
    isReady,
    initialize,
    refreshStatus,
    setup,
    login,
    logout,
    changePassword,
    setProtection,
    handleUnauthorized,
    startCoordination,
    stopCoordination,
  };
});
