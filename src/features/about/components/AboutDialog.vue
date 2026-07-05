<script setup lang="ts">
import { NButton, NCard, NModal } from "naive-ui";
import type { AppInfo, AppUpdateCheck } from "../../../types/app";

const props = defineProps<{
  show: boolean;
  appInfo: AppInfo | null;
  updateCheck: AppUpdateCheck | null;
  isCheckingUpdate?: boolean;
}>();

const emit = defineEmits<{
  "update:show": [show: boolean];
  checkUpdate: [];
}>();

function updateShow(show: boolean) {
  emit("update:show", show);
}

function closeDialog() {
  updateShow(false);
}

function checkUpdate() {
  emit("checkUpdate");
}
</script>

<template>
  <NModal :show="props.show" @update:show="updateShow">
    <NCard class="about-dialog" role="dialog" aria-modal="true">
      <template #header>
        <div>
          <p class="eyebrow">About</p>
          <h2>关于 {{ props.appInfo?.name ?? "Motrix" }}</h2>
        </div>
      </template>
      <template #header-extra>
        <NButton quaternary circle title="关闭" aria-label="关闭" @click="closeDialog">×</NButton>
      </template>

      <div class="about-placeholder">
        <p>当前版本：v{{ props.appInfo?.version ?? "--" }}</p>
        <p>维护者：{{ props.appInfo?.maintainer ?? "--" }}</p>
        <NButton :loading="props.isCheckingUpdate" @click="checkUpdate">检查更新</NButton>
        <p v-if="props.updateCheck">{{ props.updateCheck.message }}</p>
      </div>
    </NCard>
  </NModal>
</template>

<style scoped>
.about-dialog {
  width: min(720px, calc(100vw - 48px));
  max-height: calc(var(--app-viewport-height) - 48px);
  overflow: auto;
}

.eyebrow,
h2,
p {
  margin: 0;
}

.eyebrow {
  margin-bottom: 6px;
  color: #66e39a;
  font-size: 12px;
  font-weight: 800;
  letter-spacing: 0.1em;
  text-transform: uppercase;
}

.about-placeholder {
  display: grid;
  gap: 12px;
  color: #dbe3d8;
}
</style>
