<script setup lang="ts">
import SidebarNav from "./SidebarNav.vue";
import Topbar from "./Topbar.vue";
import type { AppInfo } from "../types/app";
import type { MainNavCategory } from "../types/navigation";
import type { TopbarActionStates } from "../types/topbar";

defineProps<{
  appInfo: AppInfo | null;
  activeCategory: MainNavCategory;
  topbarActions?: TopbarActionStates;
}>();

const emit = defineEmits<{
  create: [];
  refresh: [];
  pauseVisible: [];
  resumeVisible: [];
  deleteVisible: [];
  openAbout: [];
  openDiagnostics: [];
  openHelp: [];
  openSettings: [];
  selectCategory: [category: MainNavCategory];
}>();

function createTask() {
  emit("create");
}

function refreshTasks() {
  emit("refresh");
}

function pauseVisibleTasks() {
  emit("pauseVisible");
}

function resumeVisibleTasks() {
  emit("resumeVisible");
}

function deleteVisibleTasks() {
  emit("deleteVisible");
}

function openAbout() {
  emit("openAbout");
}

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
      @open-about="openAbout"
      @open-help="openHelp"
      @open-settings="openSettings"
      @select-category="selectCategory"
    />

    <section class="main-area shell-main-area">
      <Topbar
        :active-category="activeCategory"
        :action-states="topbarActions"
        @create="createTask"
        @refresh="refreshTasks"
        @pause-visible="pauseVisibleTasks"
        @resume-visible="resumeVisibleTasks"
        @delete-visible="deleteVisibleTasks"
        @open-about="openAbout"
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
  color: var(--app-text-secondary);
  background: var(--app-color-shell);
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
  background: var(--app-color-surface);
}

.content-stage {
  width: 100%;
  max-width: 100%;
  min-width: 0;
  min-height: 0;
  overflow: hidden;
  position: relative;
}

@media (min-width: 1024px) {
  .window-shell {
    grid-template-columns: var(--app-desktop-sidebar-width) minmax(0, 1fr);
  }

  .main-area {
    grid-template-rows: var(--app-desktop-topbar-height) minmax(0, 1fr);
  }
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
