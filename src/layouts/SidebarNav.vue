<script setup lang="ts">
import type { AppInfo } from "../types/app";

defineProps<{
  appInfo: AppInfo | null;
}>();

const emit = defineEmits<{
  openSettings: [];
}>();

function openSettings() {
  emit("openSettings");
}
</script>

<template>
  <aside class="sidebar">
    <div class="brand">
      <div class="brand-mark" />
      <div>
        <strong>{{ appInfo?.name ?? "Motrix FNOS" }}</strong>
        <span>v{{ appInfo?.version ?? "2.1.0" }}</span>
      </div>
    </div>

    <nav class="category-list" aria-label="任务分类">
      <button type="button" class="active">
        <span class="nav-icon">⇩</span>
        <span>Downloading</span>
      </button>
      <button type="button">
        <span class="nav-icon">✓</span>
        <span>Completed</span>
      </button>
      <button type="button">
        <span class="nav-icon">Ⅱ</span>
        <span>Stopped</span>
      </button>
      <button type="button" class="nav-spaced">
        <span class="nav-icon">♜</span>
        <span>Trash</span>
      </button>
      <button type="button">
        <span class="nav-icon">♧</span>
        <span>Extensions</span>
      </button>
    </nav>

    <div class="sidebar-footer">
      <button type="button" @click="openSettings">
        <span class="nav-icon">⚙</span>
        <span>Settings</span>
      </button>
      <button type="button">
        <span class="nav-icon">?</span>
        <span>Help</span>
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

.sidebar-footer {
  margin: 0;
  padding: 26px 8px 0;
  border-top: 1px solid #39443b;
}
</style>
