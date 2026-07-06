<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "../i18n";
import type { MainNavCategory } from "../types/navigation";
import { getMainNavLabelKey } from "./navigation";

const props = defineProps<{
  activeCategory: MainNavCategory;
}>();

const emit = defineEmits<{
  openAbout: [];
  openDiagnostics: [];
  openHelp: [];
  openSettings: [];
}>();

const { t } = useI18n();
const activeCategoryLabel = computed(() => t(getMainNavLabelKey(props.activeCategory)));

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
      <button type="button" :title="t('topbar.filter')" :aria-label="t('topbar.filter')">≡</button>
      <button type="button" :title="t('topbar.sort')" :aria-label="t('topbar.sort')">≡</button>
      <button type="button" :title="t('topbar.diagnostics')" :aria-label="t('topbar.diagnostics')" @click="openDiagnostics">⋮</button>
    </div>
    <div class="topbar-actions mobile-actions">
      <button type="button" :title="t('nav.settings')" :aria-label="t('nav.settings')" @click="openSettings">⚙</button>
      <button type="button" :title="t('nav.help')" :aria-label="t('nav.help')" @click="openHelp">?</button>
      <button type="button" :title="t('nav.about')" :aria-label="t('nav.about')" @click="openAbout">i</button>
      <button type="button" :title="t('topbar.diagnostics')" :aria-label="t('topbar.diagnostics')" @click="openDiagnostics">⋮</button>
    </div>
  </header>
</template>

<style scoped>
.topbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  border-bottom: 1px solid #324036;
  background: var(--app-color-surface);
}

.topbar-title {
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
  padding-left: 26px;
}

.topbar-title span {
  color: var(--app-text-dim);
  font-size: 12px;
  line-height: 1.2;
}

.topbar-title strong {
  color: #f1f6f1;
  font-size: 18px;
  line-height: 1.2;
  overflow-wrap: anywhere;
}

.topbar-actions {
  display: flex;
  align-items: center;
  gap: 16px;
  padding-right: 26px;
}

.topbar-actions button {
  min-width: var(--app-touch-target-min);
  min-height: var(--app-touch-target-min);
  border: 0;
  padding: 4px;
  color: #cfd8ce;
  background: transparent;
  font-size: 23px;
  line-height: 1;
  cursor: pointer;
}

@media (min-width: 768px) {
  .topbar-title {
    visibility: hidden;
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
    padding-top: var(--app-safe-area-top);
  }

  .topbar-title {
    gap: 3px;
    padding-left: var(--app-mobile-page-gutter);
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
    padding-right: var(--app-mobile-page-gutter);
  }

  .topbar-actions button {
    min-width: var(--app-touch-target-min);
    min-height: var(--app-touch-target-min);
    border-radius: var(--app-radius-sm);
    font-size: 20px;
  }
}
</style>
