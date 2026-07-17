<script setup lang="ts">
import AppIcon from "../components/AppIcon.vue";
import type { AppInfo } from "../types/app";
import type { MainNavCategory } from "../types/navigation";
import { useI18n } from "../i18n";
import { mainNavItems } from "./navigation";

defineProps<{
  appInfo: AppInfo | null;
  activeCategory: MainNavCategory;
  logoutLoading?: boolean;
}>();

const emit = defineEmits<{
  openAbout: [];
  openDiagnostics: [];
  openHelp: [];
  openSettings: [];
  logout: [];
  selectCategory: [category: MainNavCategory];
}>();

const { t } = useI18n();

function openAbout() {
  emit("openAbout");
}

function openHelp() {
  emit("openHelp");
}

function openDiagnostics() {
  emit("openDiagnostics");
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
  <aside class="sidebar">
    <div class="sidebar-heading">
      <strong>{{ t("nav.taskList") }}</strong>
    </div>

    <nav class="category-list" :aria-label="t('nav.categories')">
      <button
        v-for="item in mainNavItems"
        :key="item.key"
        type="button"
        :class="{ active: activeCategory === item.key, 'nav-spaced': item.spaced }"
        :aria-current="activeCategory === item.key ? 'page' : undefined"
        :aria-label="t(item.labelKey)"
        @click="selectCategory(item.key)"
      >
        <AppIcon class="nav-icon" :name="item.iconName" size="1em" />
        <span class="nav-label">{{ t(item.labelKey) }}</span>
      </button>
    </nav>

    <div class="sidebar-footer">
      <button type="button" :aria-label="t('nav.settings')" @click="openSettings">
        <AppIcon class="nav-icon" name="settings" :size="18" />
        <span>{{ t("nav.settings") }}</span>
      </button>
      <button type="button" :aria-label="t('nav.help')" @click="openHelp">
        <AppIcon class="nav-icon" name="help" :size="18" />
        <span>{{ t("nav.help") }}</span>
      </button>
      <button type="button" :aria-label="t('nav.about')" @click="openAbout">
        <AppIcon class="nav-icon" name="about" :size="18" />
        <span>{{ t("nav.about") }}</span>
      </button>
      <button type="button" :aria-label="t('topbar.diagnostics')" @click="openDiagnostics">
        <AppIcon class="nav-icon" name="diagnostics" :size="18" />
        <span>{{ t("topbar.diagnostics") }}</span>
      </button>
      <button type="button" :disabled="logoutLoading" :aria-label="t('auth.logout')" @click="logout">
        <AppIcon class="nav-icon" name="logout" :size="18" />
        <span>{{ t("auth.logout") }}</span>
      </button>
    </div>
  </aside>
</template>

<style scoped src="./SidebarNav.css"></style>
