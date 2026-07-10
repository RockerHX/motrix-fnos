<script setup lang="ts">
import type { AppInfo } from "../types/app";
import type { MainNavCategory } from "../types/navigation";
import { useI18n } from "../i18n";
import { mainNavItems } from "./navigation";

defineProps<{
  appInfo: AppInfo | null;
  activeCategory: MainNavCategory;
}>();

const emit = defineEmits<{
  openAbout: [];
  openHelp: [];
  openSettings: [];
  selectCategory: [category: MainNavCategory];
}>();

const { t } = useI18n();

function openAbout() {
  emit("openAbout");
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
        <span class="nav-icon">{{ item.icon }}</span>
        <span class="nav-label">{{ t(item.labelKey) }}</span>
      </button>
    </nav>

    <div class="sidebar-footer">
      <button type="button" :aria-label="t('nav.settings')" @click="openSettings">
        <span class="nav-icon">⚙</span>
        <span>{{ t("nav.settings") }}</span>
      </button>
      <button type="button" :aria-label="t('nav.help')" @click="openHelp">
        <span class="nav-icon">?</span>
        <span>{{ t("nav.help") }}</span>
      </button>
      <button type="button" :aria-label="t('nav.about')" @click="openAbout">
        <span class="nav-icon">i</span>
        <span>{{ t("nav.about") }}</span>
      </button>
    </div>
  </aside>
</template>

<style scoped>
.sidebar {
  min-height: 0;
  display: grid;
  grid-template-rows: auto minmax(0, 1fr) auto;
  padding: 42px 14px 24px;
  border-right: 1px solid var(--app-color-border-subtle);
  background: var(--app-color-shell);
}

.sidebar-heading {
  padding: 0 0 34px;
}

.sidebar-heading strong {
  display: block;
  color: var(--app-text-strong);
  font-size: 30px;
  font-weight: 600;
  line-height: 1.2;
}

.category-list,
.sidebar-footer {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.category-list button,
.sidebar-footer button {
  display: flex;
  align-items: center;
  gap: 18px;
  width: 100%;
  min-height: 72px;
  border: 0;
  border-radius: 6px;
  padding: 10px 22px;
  color: var(--app-text-secondary);
  background: transparent;
  font: inherit;
  font-size: 22px;
  text-align: left;
  cursor: pointer;
}

.category-list button.active {
  color: var(--app-text-strong);
  background: var(--app-color-card-overlay);
}

.nav-spaced {
  margin-top: 28px;
}

.nav-icon {
  width: 28px;
  color: currentColor;
  text-align: center;
  font-size: 22px;
  font-weight: 700;
}

.nav-label {
  min-width: 0;
}

.sidebar-footer {
  display: none;
  margin: 0;
  padding: 26px 8px 0;
  border-top: 1px solid var(--app-color-border-subtle);
}

.sidebar-footer button {
  min-height: var(--app-touch-target-min);
  gap: 14px;
  padding: 10px 12px;
  color: var(--app-text-muted);
  font-size: 16px;
}

@media (max-width: 767px) {
  .sidebar {
    grid-template-rows: minmax(0, 1fr);
    padding: 8px var(--app-mobile-page-gutter) calc(8px + var(--app-safe-area-bottom));
    border-top: 1px solid #324036;
    border-right: 0;
  }

  .sidebar-heading,
  .sidebar-footer {
    display: none;
  }

  .category-list {
    display: grid;
    grid-template-columns: repeat(5, minmax(0, 1fr));
    gap: 6px;
  }

  .category-list button {
    min-height: calc(var(--app-touch-target-min) + 4px);
    justify-content: center;
    gap: 4px;
    padding: 8px 6px;
    border-radius: var(--app-radius-sm);
    flex-direction: column;
    font-size: 12px;
    line-height: 1.2;
    text-align: center;
  }

  .nav-spaced {
    margin-top: 0;
  }

  .nav-icon {
    width: auto;
    font-size: 18px;
    line-height: 1;
  }

  .nav-label {
    display: -webkit-box;
    overflow: hidden;
    word-break: break-word;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 2;
  }
}
</style>
