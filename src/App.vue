<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, watch } from "vue";
import { getAppInfo, pingBackend } from "./services/backend";
import { disposeRuntimeEvents, initializeRuntimeEvents } from "./services/runtimeEvents";
import type { AppInfo, BackendPing } from "./types/app";
import NaiveProvider from "./app/providers/NaiveProvider.vue";
import { useSettingsStore } from "./features/settings/stores/settingsStore";
import MainWindow from "./views/MainWindow.vue";
import AuthGate from "./features/auth/components/AuthGate.vue";
import { useAuthStore } from "./features/auth/stores/authStore";
import { getAuthStatus } from "./features/auth/services/authService";

const settingsStore = useSettingsStore();
const authStore = useAuthStore();
const appInfo = ref<AppInfo | null>(null);
const backendPing = ref<BackendPing | null>(null);
const errorMessage = ref("");
let businessStarted = false;

async function refreshBackendStatus() {
  errorMessage.value = "";

  try {
    const [info, ping] = await Promise.all([getAppInfo(), pingBackend()]);
    appInfo.value = info;
    backendPing.value = ping;
  } catch (error) {
    errorMessage.value = error instanceof Error ? error.message : String(error);
  }
}

onMounted(() => {
  authStore.startCoordination();
  void authStore.initialize();
});

watch(
  () => authStore.isReady,
  (ready) => {
    if (ready) {
      startBusiness();
    } else {
      stopBusiness();
    }
  },
  { immediate: true },
);

function startBusiness() {
  if (businessStarted) return;
  businessStarted = true;
  initializeRuntimeEvents({
    checkAuth: getAuthStatus,
    onUnauthorized: authStore.handleUnauthorizedStatus,
  });
  void settingsStore.loadConfig();
  void refreshBackendStatus();
}

function stopBusiness() {
  if (!businessStarted) return;
  businessStarted = false;
  disposeRuntimeEvents();
  appInfo.value = null;
  backendPing.value = null;
  errorMessage.value = "";
}

onBeforeUnmount(() => {
  stopBusiness();
  authStore.stopCoordination();
});
</script>

<template>
  <NaiveProvider>
    <MainWindow v-if="authStore.isReady" :app-info="appInfo" :backend-ping="backendPing" :error-message="errorMessage" />
    <AuthGate v-else />
  </NaiveProvider>
</template>
