<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from "vue";
import { getAppInfo, pingBackend } from "./services/backend";
import { disposeRuntimeEvents, initializeRuntimeEvents } from "./services/runtimeEvents";
import type { AppInfo, BackendPing } from "./types/app";
import NaiveProvider from "./app/providers/NaiveProvider.vue";
import { useSettingsStore } from "./features/settings/stores/settingsStore";
import MainWindow from "./views/MainWindow.vue";

const settingsStore = useSettingsStore();
const appInfo = ref<AppInfo | null>(null);
const backendPing = ref<BackendPing | null>(null);
const errorMessage = ref("");

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
  initializeRuntimeEvents();
  void settingsStore.loadConfig();
  void refreshBackendStatus();
});

onBeforeUnmount(() => {
  disposeRuntimeEvents();
});
</script>

<template>
  <NaiveProvider>
    <MainWindow :app-info="appInfo" :backend-ping="backendPing" :error-message="errorMessage" />
  </NaiveProvider>
</template>
