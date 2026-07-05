<script setup lang="ts">
import { computed, reactive, ref, watch } from "vue";
import {
  NButton,
  NCard,
  NForm,
  NFormItem,
  NInput,
  NInputNumber,
  NModal,
  NSelect,
  NSpace,
  NText,
  useMessage,
} from "naive-ui";
import { useSettingsStore } from "../stores/settingsStore";
import { supportedLanguages, useI18n } from "../../../i18n";
import type { AppConfig } from "../../../types/settings";

const props = defineProps<{
  show: boolean;
}>();

const emit = defineEmits<{
  "update:show": [value: boolean];
}>();

const message = useMessage();
const settingsStore = useSettingsStore();
const { t } = useI18n();
const isJsonRpcTokenVisible = ref(false);
const form = reactive({
  defaultDownloadDir: "",
  maxConcurrentDownloads: 5,
  downloadLimitKb: 0,
  uploadLimitKb: 0,
  language: "zh-CN" as AppConfig["language"],
  jsonRpcToken: "",
});
const accessiblePathOptions = computed(() =>
  settingsStore.accessiblePaths.map((path) => ({
    label: path,
    value: path,
  })),
);
const languageOptions = computed(() =>
  supportedLanguages.map((language) => ({
    label: language === "zh-CN" ? t("language.zhCN") : t("language.enUS"),
    value: language,
  })),
);
const isDefaultDownloadDirUnauthorized = computed(
  () =>
    settingsStore.accessiblePaths.length > 0 &&
    !!form.defaultDownloadDir &&
    !settingsStore.accessiblePaths.includes(form.defaultDownloadDir),
);
const defaultDownloadDirMessage = computed(() => {
  if (settingsStore.accessiblePathsError) {
    return t("settings.defaultDownloadDir.failed", { message: settingsStore.accessiblePathsError });
  }
  if (settingsStore.accessiblePaths.length === 0) {
    return t("settings.defaultDownloadDir.empty");
  }
  if (isDefaultDownloadDirUnauthorized.value) {
    return t("settings.defaultDownloadDir.unauthorized");
  }
  return t("settings.defaultDownloadDir.help");
});
const canSave = computed(
  () =>
    !settingsStore.isSaving &&
    !settingsStore.isLoading &&
    !settingsStore.isLoadingAccessiblePaths &&
    !isDefaultDownloadDirUnauthorized.value,
);

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
    const [config] = await Promise.all([settingsStore.loadConfig(), settingsStore.loadAccessiblePaths()]);
    applyConfig(config);
  } catch (error) {
    message.error(getErrorMessage(error));
  }
}

async function saveSettings() {
  try {
    const config = await settingsStore.saveConfig(buildPayload());
    applyConfig(config);
    message.success(t("settings.saved"));
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
  form.language = config.language;
  form.jsonRpcToken = config.jsonRpcToken || "";
}

function buildPayload(): AppConfig {
  return {
    defaultDownloadDir: form.defaultDownloadDir,
    maxConcurrentDownloads: Math.trunc(form.maxConcurrentDownloads || 1),
    downloadLimit: kbToBytes(form.downloadLimitKb),
    uploadLimit: kbToBytes(form.uploadLimitKb),
    autoStartEnabled: false,
    notificationsEnabled: false,
    language: form.language,
    jsonRpcToken: form.jsonRpcToken,
  };
}

function toggleJsonRpcTokenVisible() {
  isJsonRpcTokenVisible.value = !isJsonRpcTokenVisible.value;
}

function generateJsonRpcToken() {
  const bytes = new Uint8Array(32);
  crypto.getRandomValues(bytes);
  form.jsonRpcToken = Array.from(bytes, (byte) => byte.toString(16).padStart(2, "0")).join("");
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
  return message || t("settings.failed");
}
</script>

<template>
  <NModal :show="show" :mask-closable="!settingsStore.isSaving" @update:show="emit('update:show', $event)">
    <NCard class="settings-card" role="dialog" aria-modal="true" :title="t('settings.title')">
      <NForm label-placement="left" label-width="150px" :disabled="settingsStore.isLoading">
        <NFormItem
          :label="t('settings.defaultDownloadDir')"
          :feedback="defaultDownloadDirMessage"
          :validation-status="isDefaultDownloadDirUnauthorized || settingsStore.accessiblePathsError ? 'warning' : undefined"
        >
          <NSelect
            v-model:value="form.defaultDownloadDir"
            :options="accessiblePathOptions"
            :loading="settingsStore.isLoadingAccessiblePaths"
            :placeholder="t('settings.defaultDownloadDir.placeholder')"
            filterable
          />
        </NFormItem>

        <NFormItem :label="t('settings.language')">
          <NSelect v-model:value="form.language" :options="languageOptions" />
        </NFormItem>

        <NFormItem :label="t('settings.background')">
          <NText depth="3">{{ t("settings.background.help") }}</NText>
        </NFormItem>

        <NFormItem :label="t('settings.jsonRpcToken')" :feedback="t('settings.jsonRpcToken.help')">
          <NInput
            v-model:value="form.jsonRpcToken"
            :type="isJsonRpcTokenVisible ? 'text' : 'password'"
            clearable
            :placeholder="t('settings.jsonRpcToken.placeholder')"
          >
            <template #suffix>
              <NSpace :size="4">
                <NButton size="tiny" quaternary @click.stop="toggleJsonRpcTokenVisible">
                  {{ isJsonRpcTokenVisible ? t("settings.jsonRpcToken.hide") : t("settings.jsonRpcToken.show") }}
                </NButton>
                <NButton size="tiny" quaternary @click.stop="generateJsonRpcToken">
                  {{ t("settings.jsonRpcToken.generate") }}
                </NButton>
              </NSpace>
            </template>
          </NInput>
        </NFormItem>

        <NFormItem :label="t('settings.maxConcurrentDownloads')">
          <NInputNumber v-model:value="form.maxConcurrentDownloads" :min="1" :max="64" :step="1" />
        </NFormItem>

        <NFormItem :label="t('settings.downloadLimit')">
          <NInputNumber v-model:value="form.downloadLimitKb" :min="0" :step="128">
            <template #suffix>KB/s</template>
          </NInputNumber>
        </NFormItem>

        <NFormItem :label="t('settings.uploadLimit')">
          <NInputNumber v-model:value="form.uploadLimitKb" :min="0" :step="128">
            <template #suffix>KB/s</template>
          </NInputNumber>
        </NFormItem>
      </NForm>

      <template #footer>
        <NSpace justify="end">
          <NButton :disabled="settingsStore.isSaving" @click="closeDialog">{{ t("common.cancel") }}</NButton>
          <NButton type="primary" :loading="settingsStore.isSaving" :disabled="!canSave" @click="saveSettings">
            {{ t("common.save") }}
          </NButton>
        </NSpace>
      </template>
    </NCard>
  </NModal>
</template>

<style scoped>
.settings-card {
  width: min(620px, calc(100vw - 48px));
}

.setting-stack {
  width: 100%;
}
</style>
