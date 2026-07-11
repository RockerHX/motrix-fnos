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
  openAbout: [];
  openDiagnostics: [];
  openHelp: [];
  openSettings: [];
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
</script>

<template>
  <header class="topbar">
    <div class="topbar-title">
      <span>Motrix</span>
      <strong>{{ activeCategoryLabel }}</strong>
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
        type="button"
        :disabled="isActionDisabled('deleteVisible')"
        :title="actionTitle('deleteVisible', t('topbar.deleteVisible'))"
        :aria-label="t('topbar.deleteVisible')"
        @click="deleteVisibleTasks"
      >
        <AppIcon name="close" :size="16" />
      </button>
    </div>
    <div class="topbar-actions mobile-actions">
      <button type="button" :title="t('nav.settings')" :aria-label="t('nav.settings')" @click="openSettings"><AppIcon name="settings" :size="18" /></button>
      <button type="button" :title="t('nav.help')" :aria-label="t('nav.help')" @click="openHelp"><AppIcon name="help" :size="18" /></button>
      <button type="button" :title="t('nav.about')" :aria-label="t('nav.about')" @click="openAbout"><AppIcon name="about" :size="18" /></button>
      <button type="button" :title="t('topbar.diagnostics')" :aria-label="t('topbar.diagnostics')" @click="openDiagnostics"><AppIcon name="diagnostics" :size="18" /></button>
    </div>
  </header>
</template>

<style scoped>
.topbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  border-bottom: 1px solid var(--app-color-border-subtle);
  padding: 0 var(--app-desktop-content-gutter-x);
  background: var(--app-color-surface);
}

.topbar-title {
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.topbar-title span {
  color: var(--app-text-dim);
  font-size: 12px;
  line-height: 1.2;
}

.topbar-title strong {
  color: var(--app-text-strong);
  font-size: 20px;
  font-weight: 600;
  line-height: 1.2;
  overflow-wrap: anywhere;
}

.topbar-actions {
  display: flex;
  align-items: center;
  gap: 6px;
}

.topbar-actions > button {
  width: var(--app-toolbar-button-size);
  min-width: var(--app-toolbar-button-size);
  height: var(--app-toolbar-button-size);
  min-height: var(--app-toolbar-button-size);
  display: grid;
  place-items: center;
  border: 0;
  border-radius: 9px;
  padding: 0;
  color: var(--app-text-muted);
  background: transparent;
  font: inherit;
  font-size: 16px;
  line-height: 1;
  cursor: pointer;
}

.topbar-actions > button:hover,
.topbar-actions > button:focus-visible {
  color: var(--app-text-strong);
  background: var(--app-color-card-overlay);
  outline: none;
}

.topbar-actions > button:disabled {
  cursor: not-allowed;
  opacity: 0.45;
}

.topbar-actions > button:disabled:hover {
  color: var(--app-text-secondary);
  background: transparent;
}

.topbar-actions > .topbar-primary-button {
  width: var(--app-toolbar-primary-button-size);
  min-width: var(--app-toolbar-primary-button-size);
  height: var(--app-toolbar-primary-button-size);
  min-height: var(--app-toolbar-primary-button-size);
  border-radius: var(--app-radius-pill);
  color: #101710;
  background: var(--app-text-accent);
  font-size: 20px;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.22);
}

.topbar-actions > .topbar-primary-button:hover,
.topbar-actions > .topbar-primary-button:focus-visible {
  color: #101710;
  background: var(--app-text-accent-soft);
}

.topbar-actions > .topbar-primary-button:disabled,
.topbar-actions > .topbar-primary-button:disabled:hover {
  color: #101710;
  background: var(--app-text-accent);
  box-shadow: none;
}

@media (min-width: 768px) {
  .topbar-title span {
    display: none;
  }

  .mobile-actions {
    display: none;
  }
}

@media (max-width: 767px) {
  .desktop-actions {
    display: none;
  }

  .topbar {
    min-height: calc(56px + var(--app-safe-area-top));
    padding: var(--app-safe-area-top) var(--app-mobile-page-gutter) 0;
  }

  .topbar-title {
    gap: 3px;
  }

  .topbar-title span {
    font-size: 11px;
  }

  .topbar-title strong {
    font-size: 17px;
    line-height: 1.25;
  }

  .topbar-actions {
    gap: 10px;
  }

  .topbar-actions > button {
    min-width: var(--app-touch-target-min);
    min-height: var(--app-touch-target-min);
    border-radius: var(--app-radius-sm);
    font-size: 20px;
  }
}
</style>
