<script setup lang="ts">
import SidebarNav from "./SidebarNav.vue";
import Topbar from "./Topbar.vue";
import type { AppInfo } from "../types/app";

defineProps<{
  appInfo: AppInfo | null;
}>();

const emit = defineEmits<{
  openDiagnostics: [];
}>();

function openDiagnostics() {
  emit("openDiagnostics");
}
</script>

<template>
  <div class="window-shell">
    <SidebarNav :app-info="appInfo" />

    <section class="main-area">
      <Topbar @open-diagnostics="openDiagnostics" />
      <main class="content-stage">
        <slot />
      </main>
    </section>

    <slot name="overlay" />
  </div>
</template>

<style scoped>
.window-shell {
  position: relative;
  height: 100vh;
  overflow: hidden;
  display: grid;
  grid-template-columns: 220px minmax(0, 1fr);
  color: #d7dfd8;
  background: #121212;
}

.main-area {
  min-width: 0;
  min-height: 0;
  display: grid;
  grid-template-rows: 52px minmax(0, 1fr);
  background: #151515;
}

.content-stage {
  min-height: 0;
  overflow: hidden;
  position: relative;
}
</style>
