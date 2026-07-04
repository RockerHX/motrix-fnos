<script setup lang="ts">
import { useI18n } from "../i18n";

const emit = defineEmits<{
  openDiagnostics: [];
}>();

const { t } = useI18n();

function openDiagnostics() {
  emit("openDiagnostics");
}
</script>

<template>
  <header class="topbar">
    <div class="topbar-spacer" />
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
  justify-content: flex-end;
  border-bottom: 1px solid #324036;
  background: #151515;
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
</style>
