<script setup lang="ts">
import { reactive, watch } from "vue";
import {
  NButton,
  NCard,
  NForm,
  NFormItem,
  NInputNumber,
  NModal,
  NSpace,
  NSwitch,
  NText,
  useMessage,
} from "naive-ui";
import { useSettingsStore } from "../stores/settingsStore";
import type { AppConfig } from "../../../types/settings";

const props = defineProps<{
  show: boolean;
}>();

const emit = defineEmits<{
  "update:show": [value: boolean];
}>();

const message = useMessage();
const settingsStore = useSettingsStore();
const form = reactive({
  defaultDownloadDir: "",
  maxConcurrentDownloads: 5,
  downloadLimitKb: 0,
  uploadLimitKb: 0,
  autoStartEnabled: false,
  notificationsEnabled: false,
});

watch(
  () => props.show,
  (show) => {
    if (show) {
      void loadSettings();
    }
  },
);

async function loadSettings() {
  try {
    const config = await settingsStore.loadConfig();
    applyConfig(config);
  } catch (error) {
    message.error(getErrorMessage(error));
  }
}

async function saveSettings() {
  try {
    const config = await settingsStore.saveConfig(buildPayload());
    applyConfig(config);
    message.success("设置已保存");
    closeDialog();
  } catch (error) {
    message.error(getErrorMessage(error));
  }
}

function applyConfig(config: AppConfig) {
  form.defaultDownloadDir = config.defaultDownloadDir;
  form.maxConcurrentDownloads = config.maxConcurrentDownloads;
  form.downloadLimitKb = bytesToKb(config.downloadLimit);
  form.uploadLimitKb = bytesToKb(config.uploadLimit);
  form.autoStartEnabled = config.autoStartEnabled;
  form.notificationsEnabled = config.notificationsEnabled;
}

function buildPayload(): AppConfig {
  return {
    defaultDownloadDir: form.defaultDownloadDir,
    maxConcurrentDownloads: Math.trunc(form.maxConcurrentDownloads || 1),
    downloadLimit: kbToBytes(form.downloadLimitKb),
    uploadLimit: kbToBytes(form.uploadLimitKb),
    autoStartEnabled: form.autoStartEnabled,
    notificationsEnabled: form.notificationsEnabled,
  };
}

function closeDialog() {
  emit("update:show", false);
}

function bytesToKb(value: number) {
  return Math.floor(Math.max(0, value) / 1024);
}

function kbToBytes(value: number) {
  return Math.floor(Math.max(0, value || 0) * 1024);
}

function getErrorMessage(error: unknown) {
  if (error instanceof Error) {
    return error.message;
  }

  const message = String(error);
  return message || "设置操作失败，请稍后重试";
}
</script>

<template>
  <NModal :show="show" :mask-closable="!settingsStore.isSaving" @update:show="emit('update:show', $event)">
    <NCard class="settings-card" role="dialog" aria-modal="true" title="设置">
      <NForm label-placement="left" label-width="150px" :disabled="settingsStore.isLoading">
        <NFormItem label="后台驻留">
          <NText depth="3">关闭窗口后隐藏到后台，下载任务继续运行。</NText>
        </NFormItem>

        <NFormItem label="开机自启">
          <NSwitch v-model:value="form.autoStartEnabled" />
        </NFormItem>

        <NFormItem label="下载通知">
          <NSwitch v-model:value="form.notificationsEnabled" />
        </NFormItem>

        <NFormItem label="最大并发下载数">
          <NInputNumber v-model:value="form.maxConcurrentDownloads" :min="1" :max="64" :step="1" />
        </NFormItem>

        <NFormItem label="下载限速">
          <NInputNumber v-model:value="form.downloadLimitKb" :min="0" :step="128">
            <template #suffix>KB/s</template>
          </NInputNumber>
        </NFormItem>

        <NFormItem label="上传限速">
          <NInputNumber v-model:value="form.uploadLimitKb" :min="0" :step="128">
            <template #suffix>KB/s</template>
          </NInputNumber>
        </NFormItem>
      </NForm>

      <template #footer>
        <NSpace justify="end">
          <NButton :disabled="settingsStore.isSaving" @click="closeDialog">取消</NButton>
          <NButton type="primary" :loading="settingsStore.isSaving" @click="saveSettings">保存</NButton>
        </NSpace>
      </template>
    </NCard>
  </NModal>
</template>

<style scoped>
.settings-card {
  width: min(620px, calc(100vw - 48px));
}
</style>
