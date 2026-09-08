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
  logoutLoading?: boolean;
}>();

const emit = defineEmits<{
  create: [];
  refresh: [];
  pauseVisible: [];
  resumeVisible: [];
  deleteVisible: [];
  clearTrash: [];
  openAbout: [];
  openDiagnostics: [];
  openHelp: [];
  openSettings: [];
  logout: [];
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

function clearTrash() {
  emit("clearTrash");
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

function logout() {
  emit("logout");
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
      :logout-loading="logoutLoading"
      @open-about="openAbout"
      @open-diagnostics="openDiagnostics"
      @open-help="openHelp"
      @open-settings="openSettings"
      @logout="logout"
      @select-category="selectCategory"
    />

    <section class="main-area shell-main-area">
      <Topbar
        :active-category="activeCategory"
        :action-states="topbarActions"
        :logout-loading="logoutLoading"
        @create="createTask"
        @refresh="refreshTasks"
        @pause-visible="pauseVisibleTasks"
        @resume-visible="resumeVisibleTasks"
        @delete-visible="deleteVisibleTasks"
        @clear-trash="clearTrash"
        @open-about="openAbout"
        @open-diagnostics="openDiagnostics"
        @open-help="openHelp"
        @open-settings="openSettings"
        @logout="logout"
      />
      <main class="content-stage">
        <slot />
      </main>
    </section>

    <slot name="overlay" />
  </div>
</template>

<style scoped src="./AppShell.css"></style>
