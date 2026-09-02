<script setup lang="ts">
import { defineAsyncComponent, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { createBootstrapController } from "./app/bootstrap";
import { getAppInfo, pingBackend } from "./services/backend";
import { disposeRuntimeEvents, initializeRuntimeEvents } from "./services/runtimeEvents";
import type { AppInfo, BackendPing } from "./types/app";
import NaiveProvider from "./app/providers/NaiveProvider.vue";
import { useSettingsStore } from "./features/settings/stores/settingsStore";
import AuthGate from "./features/auth/components/AuthGate.vue";
import { useAuthStore } from "./features/auth/stores/authStore";
import { getAuthStatus } from "./features/auth/services/authService";
import { createFnosPlatformController } from "./app/hostPlatform";

const settingsStore = useSettingsStore();
const authStore = useAuthStore();
const loadMainWindow = () => import("./views/MainWindow.vue");
const MainWindow = defineAsyncComponent(async () => (await loadMainWindow()).default);
const bootstrapController = createBootstrapController();
const appInfo = ref<AppInfo | null>(null);
const backendPing = ref<BackendPing | null>(null);
const errorMessage = ref("");
let businessStarted = false;
const platformController = createFnosPlatformController(() => !authStore.isReady);

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
  void platformController.initialize();
  authStore.startCoordination();
  bootstrapController.startConfirmation();
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

watch(
  () => authStore.phase,
  (phase) => {
    if (phase !== "loading") void revealResolvedAuthPhase(phase);
  },
  { flush: "post" },
);

async function revealResolvedAuthPhase(phase: typeof authStore.phase) {
  if (phase === "ready") {
    try {
      await loadMainWindow();
    } catch {
      // 启动层退出后，由异步组件继续暴露自身加载失败状态。
    }
  }
  await nextTick();
  bootstrapController.finish();
}

function startBusiness() {
  if (businessStarted) return;
  businessStarted = true;
  initializeRuntimeEvents({
    checkAuth: getAuthStatus,
    onUnauthorized: authStore.handleUnauthorizedStatus,
    getAccessToken: authStore.getAccessToken,
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
  platformController.dispose();
  stopBusiness();
  authStore.stopCoordination();
  bootstrapController.dispose();
});
</script>

<template>
  <NaiveProvider>
    <MainWindow v-if="authStore.isReady" :app-info="appInfo" :backend-ping="backendPing" :error-message="errorMessage" />
    <AuthGate v-else />
  </NaiveProvider>
</template>
