<script setup lang="ts">
const emit = defineEmits<{
  openDiagnostics: [];
}>();

function openDiagnostics() {
  emit("openDiagnostics");
}
</script>

<template>
  <header class="topbar">
    <div class="topbar-spacer" />
    <div class="topbar-actions">
      <button type="button" title="筛选">≡</button>
      <button type="button" title="排序">≡</button>
      <button type="button" title="诊断" @click="openDiagnostics">⋮</button>
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
