<script setup lang="ts">
import { onMounted, ref } from "vue";
import { getAppInfo, pingBackend } from "./services/backend";
import type { AppInfo, BackendPing } from "./types/app";
import MainWindow from "./views/MainWindow.vue";

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
  void refreshBackendStatus();
});
</script>

<template>
  <MainWindow :app-info="appInfo" :backend-ping="backendPing" :error-message="errorMessage" />
</template>

<style>
:root {
  font-family: Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
  color: #dce8e2;
  background: #0b0f0e;
  font-synthesis: none;
  text-rendering: optimizeLegibility;
  -webkit-font-smoothing: antialiased;
  -moz-osx-font-smoothing: grayscale;
}

* {
  box-sizing: border-box;
}

html,
body,
#app {
  width: 100%;
  height: 100%;
}

body {
  overflow: hidden;
  margin: 0;
}
</style>
