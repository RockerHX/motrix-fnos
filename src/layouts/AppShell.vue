<script setup lang="ts">
import SidebarNav from "./SidebarNav.vue";
import Topbar from "./Topbar.vue";
import type { AppInfo } from "../types/app";
import type { MainNavCategory } from "../types/navigation";

defineProps<{
  appInfo: AppInfo | null;
  activeCategory: MainNavCategory;
}>();

const emit = defineEmits<{
  openDiagnostics: [];
  openHelp: [];
  openSettings: [];
  selectCategory: [category: MainNavCategory];
}>();

function openDiagnostics() {
  emit("openDiagnostics");
}

function openHelp() {
  emit("openHelp");
}

function openSettings() {
  emit("openSettings");
}

function selectCategory(category: MainNavCategory) {
  emit("selectCategory", category);
}
</script>

<template>
  <div class="window-shell">
    <SidebarNav
      class="shell-sidebar"
      :app-info="appInfo"
      :active-category="activeCategory"
      @open-help="openHelp"
      @open-settings="openSettings"
      @select-category="selectCategory"
    />

    <section class="main-area shell-main-area">
      <Topbar
        :active-category="activeCategory"
        @open-diagnostics="openDiagnostics"
        @open-help="openHelp"
        @open-settings="openSettings"
      />
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
  height: var(--app-viewport-height);
  min-height: var(--app-viewport-height);
  overflow: hidden;
  display: grid;
  grid-template-columns: 220px minmax(0, 1fr);
  grid-template-areas: "sidebar main";
  color: #d7dfd8;
  background: #121212;
}

.shell-sidebar {
  grid-area: sidebar;
  min-width: 0;
}

.main-area {
  grid-area: main;
  width: 100%;
  max-width: 100%;
  min-width: 0;
  min-height: 0;
  display: grid;
  grid-template-rows: 52px minmax(0, 1fr);
  background: #151515;
}

.content-stage {
  width: 100%;
  max-width: 100%;
  min-width: 0;
  min-height: 0;
  overflow: hidden;
  position: relative;
}

@media (max-width: 767px) {
  .window-shell {
    grid-template-columns: minmax(0, 1fr);
    grid-template-rows: minmax(0, 1fr) auto;
    grid-template-areas:
      "main"
      "sidebar";
  }

  .shell-sidebar,
  .shell-main-area {
    width: 100%;
    max-width: 100%;
  }

  .content-stage {
    overflow-x: hidden;
    overflow-y: auto;
    -webkit-overflow-scrolling: touch;
    overscroll-behavior: contain;
  }
}
</style>
