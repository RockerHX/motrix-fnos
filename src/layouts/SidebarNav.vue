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
  openHelp: [];
  openSettings: [];
  selectCategory: [category: MainNavCategory];
}>();

const { t } = useI18n();

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
    <div class="brand">
      <div class="brand-mark" />
      <div>
        <strong>{{ appInfo?.name ?? "Motrix" }}</strong>
        <span>v{{ appInfo?.version ?? "2.1.0" }}</span>
      </div>
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
    </div>
  </aside>
</template>

<style scoped>
.sidebar {
  min-height: 0;
  display: grid;
  grid-template-rows: auto minmax(0, 1fr) auto;
  padding: 22px 8px 24px;
  border-right: 1px solid #324036;
  background: #0f100f;
}

.brand {
  display: flex;
  align-items: center;
  gap: 9px;
  padding: 0 22px 30px;
}

.brand-mark {
  width: 28px;
  height: 28px;
  border-radius: 4px;
  background: #6ab75f;
}

.brand strong,
.brand span {
  display: block;
}

.brand strong {
  color: #8ef08a;
  font-size: 22px;
  font-weight: 800;
  line-height: 1;
}

.brand span {
  margin-top: 4px;
  color: #d8e0d7;
  font-size: 12px;
}

.category-list,
.sidebar-footer {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.category-list button,
.sidebar-footer button {
  display: flex;
  align-items: center;
  gap: 14px;
  width: 100%;
  min-height: var(--app-touch-target-min);
  border: 0;
  border-radius: 6px;
  padding: 10px 12px;
  color: #cfd8ce;
  background: transparent;
  font: inherit;
  font-size: 16px;
  text-align: left;
  cursor: pointer;
}

.category-list button.active {
  color: #8ef08a;
  background: #4a4b48;
}

.nav-spaced {
  margin-top: 28px;
}

.nav-icon {
  width: 22px;
  color: currentColor;
  text-align: center;
  font-weight: 800;
}

.nav-label {
  min-width: 0;
}

.sidebar-footer {
  margin: 0;
  padding: 26px 8px 0;
  border-top: 1px solid #39443b;
}

@media (max-width: 767px) {
  .sidebar {
    grid-template-rows: minmax(0, 1fr);
    padding: 8px var(--app-mobile-page-gutter) calc(8px + var(--app-safe-area-bottom));
    border-top: 1px solid #324036;
    border-right: 0;
  }

  .brand,
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
    border-radius: 12px;
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
