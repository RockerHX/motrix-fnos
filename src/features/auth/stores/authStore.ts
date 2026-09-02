import { defineStore } from "pinia";
import { computed, ref } from "vue";
import { useDebugLogStore } from "../../diagnostics/stores/debugLogStore";
import { useSettingsStore } from "../../settings/stores/settingsStore";
import { useTaskStore } from "../../tasks/stores/taskStore";
import { useJsonRpcTokenStore } from "../../settings/stores/jsonRpcTokenStore";
import { setAccessTokenProvider, setUnauthorizedHandler } from "../../../services/http";
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
import { t } from "../../../i18n";

const ACCESS_TOKEN_STORAGE_KEY = "motrix-fnos:web-access-token";

export const useAuthStore = defineStore("auth", () => {
  const phase = ref<AuthPhase>("loading");
  const enabled = ref(true);
  const authenticated = ref(false);
  const accessToken = ref<string | null>(null);
  const localStorageAvailable = ref(true);
  const errorMessage = ref("");
  const isSubmitting = ref(false);
  let channel: AuthChannel | null = null;

  const isReady = computed(() => phase.value === "ready");
  const hasAccessToken = computed(() => Boolean(accessToken.value));

  function getAccessToken() {
    return accessToken.value;
  }

  async function initialize() {
    phase.value = "loading";
    errorMessage.value = "";
    restoreAccessToken();
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
    return submit(() => setupAuth(password), true);
  }

  async function login(password: string) {
    return submit(() => loginAuth(password), true);
  }

  async function logout() {
    isSubmitting.value = true;
    try {
      await logoutAuth();
    } finally {
      lockToLogin();
      channel?.post({ type: "auth-invalidated" });
      isSubmitting.value = false;
    }
  }

  async function changePassword(payload: ChangePasswordRequest) {
    return submit(() => changeAuthPassword(payload), true);
  }

  async function setProtection(nextEnabled: boolean, currentPassword: string) {
    return submit(() => changeAuthProtection({ enabled: nextEnabled, currentPassword }), true);
  }

  async function handleUnauthorized() {
    clearSensitiveState();
    clearAccessToken();
    try {
      applyStatus(await getAuthStatus());
    } catch {
      phase.value = "login";
    }
    channel?.post({ type: "auth-invalidated" });
  }

  function handleUnauthorizedStatus(status: AuthStatus) {
    if (!status.authenticated) {
      clearAccessToken();
    }
    applyStatus(status);
    if (!isReady.value) {
      clearSensitiveState();
      channel?.post({ type: "auth-invalidated" });
    }
  }

  function startCoordination() {
    setAccessTokenProvider(() => accessToken.value);
    setUnauthorizedHandler(handleUnauthorized);
    channel ??= createAuthChannel(handleChannelMessage);
  }

  function stopCoordination() {
    setAccessTokenProvider(null);
    setUnauthorizedHandler(null);
    channel?.close();
    channel = null;
  }

  async function submit(operation: () => Promise<AuthStatus>, verifyToken: boolean) {
    isSubmitting.value = true;
    errorMessage.value = "";
    try {
      const response = await operation();
      if (response.accessToken) {
        saveAccessToken(response.accessToken);
      }
      let status = response;
      if (verifyToken) {
        try {
          status = await getAuthStatus();
        } catch (error) {
          lockToLogin();
          throw error;
        }
        if (!status.authenticated) {
          lockToLogin();
          throw new Error(t("auth.tokenVerificationFailed"));
        }
      }
      applyStatus(status);
      channel?.post({ type: "auth-updated" });
      return status;
    } finally {
      isSubmitting.value = false;
    }
  }

  async function handleChannelMessage(message: AuthChannelMessage) {
    if (message.type === "auth-invalidated") {
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
    const wasReady = isReady.value;
    enabled.value = status.enabled;
    authenticated.value = status.authenticated;
    errorMessage.value = "";
    if (status.setupRequired) {
      phase.value = "setup";
      clearAccessToken();
    } else if (status.enabled && !status.authenticated) {
      phase.value = "login";
      clearAccessToken();
    } else {
      phase.value = "ready";
    }
    if (wasReady && phase.value !== "ready") {
      clearSensitiveState();
    }
  }

  function lockToLogin() {
    clearSensitiveState();
    clearAuthState();
    clearAccessToken();
    phase.value = "login";
  }

  function clearAuthState() {
    authenticated.value = false;
  }

  function clearSensitiveState() {
    useTaskStore().clearSensitiveState();
    useSettingsStore().clearSensitiveState();
    useJsonRpcTokenStore().clearSensitiveState();
    useDebugLogStore().clearSensitiveState();
  }

  function restoreAccessToken() {
    try {
      accessToken.value = window.localStorage.getItem(ACCESS_TOKEN_STORAGE_KEY);
      localStorageAvailable.value = true;
    } catch {
      accessToken.value = null;
      localStorageAvailable.value = false;
    }
  }

  function saveAccessToken(token: string) {
    accessToken.value = token;
    try {
      window.localStorage.setItem(ACCESS_TOKEN_STORAGE_KEY, token);
      localStorageAvailable.value = true;
    } catch {
      localStorageAvailable.value = false;
    }
  }

  function clearAccessToken() {
    accessToken.value = null;
    try {
      window.localStorage.removeItem(ACCESS_TOKEN_STORAGE_KEY);
    } catch {
      localStorageAvailable.value = false;
    }
  }

  return {
    phase,
    enabled,
    authenticated,
    accessToken,
    hasAccessToken,
    getAccessToken,
    localStorageAvailable,
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
    handleUnauthorizedStatus,
    startCoordination,
    stopCoordination,
  };
});
