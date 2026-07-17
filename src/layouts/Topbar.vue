<script setup lang="ts">
import { computed } from "vue";
import AppIcon from "../components/AppIcon.vue";
import { useI18n } from "../i18n";
import type { MainNavCategory } from "../types/navigation";
import type { TopbarActionKey, TopbarActionStates } from "../types/topbar";
import { getMainNavLabelKey } from "./navigation";

const props = withDefaults(
  defineProps<{
    activeCategory: MainNavCategory;
    actionStates?: TopbarActionStates;
    logoutLoading?: boolean;
  }>(),
  {
    actionStates: () => ({}),
  },
);

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
}>();

const { t } = useI18n();
const activeCategoryLabel = computed(() => t(getMainNavLabelKey(props.activeCategory)));

function createTask() {
  if (!isActionDisabled("create")) {
    emit("create");
  }
}

function refreshTasks() {
  if (!isActionDisabled("refresh")) {
    emit("refresh");
  }
}

function pauseVisibleTasks() {
  if (!isActionDisabled("pauseVisible")) {
    emit("pauseVisible");
  }
}

function resumeVisibleTasks() {
  if (!isActionDisabled("resumeVisible")) {
    emit("resumeVisible");
  }
}

function deleteVisibleTasks() {
  if (!isActionDisabled("deleteVisible")) {
    emit("deleteVisible");
  }
}

function clearTrash() {
  if (!isActionDisabled("clearTrash")) {
    emit("clearTrash");
  }
}

function isActionDisabled(action: TopbarActionKey) {
  return Boolean(props.actionStates[action]?.disabled);
}

function actionTitle(action: TopbarActionKey, fallback: string) {
  return props.actionStates[action]?.title || fallback;
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
  if (!props.logoutLoading) emit("logout");
}
</script>

<template>
  <header class="topbar">
    <div class="topbar-title">
      <span>Motrix</span>
      <div class="topbar-title-label">
        <Transition name="app-title-switch">
          <strong :key="props.activeCategory">{{ activeCategoryLabel }}</strong>
        </Transition>
      </div>
    </div>
    <div class="topbar-actions desktop-actions">
      <button
        type="button"
        class="topbar-primary-button"
        :disabled="isActionDisabled('create')"
        :title="actionTitle('create', t('topbar.create'))"
        :aria-label="t('topbar.create')"
        @click="createTask"
      >
        <AppIcon name="plus" :size="20" />
      </button>
      <button
        type="button"
        :disabled="isActionDisabled('refresh')"
        :title="actionTitle('refresh', t('common.refresh'))"
        :aria-label="t('common.refresh')"
        @click="refreshTasks"
      >
        <AppIcon name="refresh" :size="16" />
      </button>
      <button
        type="button"
        :disabled="isActionDisabled('pauseVisible')"
        :title="actionTitle('pauseVisible', t('topbar.pauseVisible'))"
        :aria-label="t('topbar.pauseVisible')"
        @click="pauseVisibleTasks"
      >
        <AppIcon name="pause" :size="16" />
      </button>
      <button
        type="button"
        :disabled="isActionDisabled('resumeVisible')"
        :title="actionTitle('resumeVisible', t('topbar.resumeVisible'))"
        :aria-label="t('topbar.resumeVisible')"
        @click="resumeVisibleTasks"
      >
        <AppIcon name="play" :size="16" />
      </button>
      <button
        v-if="props.activeCategory !== 'trash'"
        type="button"
        :disabled="isActionDisabled('deleteVisible')"
        :title="actionTitle('deleteVisible', t('topbar.deleteVisible'))"
        :aria-label="t('topbar.deleteVisible')"
        @click="deleteVisibleTasks"
      >
        <AppIcon name="trash" :size="16" />
      </button>
      <button
        v-else
        type="button"
        :disabled="isActionDisabled('clearTrash')"
        :title="actionTitle('clearTrash', t('topbar.clearTrash'))"
        :aria-label="t('topbar.clearTrash')"
        @click="clearTrash"
      >
        <AppIcon name="trash" :size="16" />
      </button>
    </div>
    <div class="topbar-actions mobile-actions">
      <button
        v-if="props.activeCategory === 'trash'"
        type="button"
        :disabled="isActionDisabled('clearTrash')"
        :title="actionTitle('clearTrash', t('topbar.clearTrash'))"
        :aria-label="t('topbar.clearTrash')"
        @click="clearTrash"
      >
        <AppIcon name="trash" :size="18" />
      </button>
      <button type="button" :title="t('nav.settings')" :aria-label="t('nav.settings')" @click="openSettings"><AppIcon name="settings" :size="18" /></button>
      <button type="button" :title="t('nav.help')" :aria-label="t('nav.help')" @click="openHelp"><AppIcon name="help" :size="18" /></button>
      <button type="button" :title="t('nav.about')" :aria-label="t('nav.about')" @click="openAbout"><AppIcon name="about" :size="18" /></button>
      <button type="button" :title="t('topbar.diagnostics')" :aria-label="t('topbar.diagnostics')" @click="openDiagnostics"><AppIcon name="diagnostics" :size="18" /></button>
      <button type="button" :disabled="props.logoutLoading" :title="t('auth.logout')" :aria-label="t('auth.logout')" @click="logout"><AppIcon name="logout" :size="18" /></button>
    </div>
  </header>
</template>

<style scoped src="./Topbar.css"></style>
