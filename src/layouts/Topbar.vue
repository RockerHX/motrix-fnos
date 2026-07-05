<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "../i18n";
import type { MainNavCategory } from "../types/navigation";
import { getMainNavLabelKey } from "./navigation";

const props = defineProps<{
  activeCategory: MainNavCategory;
}>();

const emit = defineEmits<{
  openDiagnostics: [];
}>();

const { t } = useI18n();
const activeCategoryLabel = computed(() => t(getMainNavLabelKey(props.activeCategory)));

function openDiagnostics() {
  emit("openDiagnostics");
}
</script>

<template>
  <header class="topbar">
    <div class="topbar-title">
      <span>Motrix</span>
      <strong>{{ activeCategoryLabel }}</strong>
    </div>
    <div class="topbar-actions">
      <button type="button" :title="t('topbar.filter')">≡</button>
      <button type="button" :title="t('topbar.sort')">≡</button>
      <button type="button" :title="t('topbar.diagnostics')" @click="openDiagnostics">⋮</button>
    </div>
  </header>
</template>

<style scoped>
.topbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  border-bottom: 1px solid #324036;
  background: #151515;
}

.topbar-title {
  display: flex;
  flex-direction: column;
  gap: 2px;
  padding-left: 26px;
}

.topbar-title span {
  color: #83958e;
  font-size: 12px;
  line-height: 1.2;
}

.topbar-title strong {
  color: #f1f6f1;
  font-size: 18px;
  line-height: 1.2;
}

.topbar-actions {
  display: flex;
  align-items: center;
  gap: 16px;
  padding-right: 26px;
}

.topbar-actions button {
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
}

@media (max-width: 767px) {
  .topbar {
    min-height: calc(56px + var(--app-safe-area-top));
    padding-top: var(--app-safe-area-top);
  }

  .topbar-title {
    padding-left: var(--app-mobile-page-gutter);
  }

  .topbar-actions {
    gap: 12px;
    padding-right: var(--app-mobile-page-gutter);
  }
}
</style>
