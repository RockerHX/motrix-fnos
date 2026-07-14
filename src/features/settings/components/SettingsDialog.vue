<script setup lang="ts">
import { computed, reactive, watch } from "vue";
import {
  NButton,
  NForm,
  NFormItem,
  NInputNumber,
  NSelect,
  NText,
  useMessage,
} from "naive-ui";
import AppDialog from "../../../components/ui/AppDialog.vue";
import AppDialogActions from "../../../components/ui/AppDialogActions.vue";
import { useSettingsStore } from "../stores/settingsStore";
import { useMobileLayout } from "../../../app/composables/useMobileLayout";
import { supportedLanguages, useI18n } from "../../../i18n";
import { getErrorMessage } from "../../../app/utils/errors";
import type { AppConfig } from "../../../types/settings";
import WebAuthSettings from "../../auth/components/WebAuthSettings.vue";
import JsonRpcTokenSettings from "./JsonRpcTokenSettings.vue";

const props = defineProps<{
  show: boolean;
}>();

const emit = defineEmits<{
  "update:show": [value: boolean];
}>();

const message = useMessage();
const settingsStore = useSettingsStore();
const { t } = useI18n();
const { isMobileLayout } = useMobileLayout();
const form = reactive({
  defaultDownloadDir: "",
  maxConcurrentDownloads: 5,
  downloadLimitKb: 0,
  uploadLimitKb: 0,
  language: "zh-CN" as AppConfig["language"],
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
    message.error(getErrorMessage(error, t("settings.failed")));
  }
}

async function saveSettings() {
  try {
    const config = await settingsStore.saveConfig(buildPayload());
    applyConfig(config);
    message.success(t("settings.saved"));
    closeDialog();
  } catch (error) {
    message.error(getErrorMessage(error, t("settings.failed")));
  }
}

function applyConfig(config: AppConfig) {
  form.defaultDownloadDir = config.defaultDownloadDir;
  form.maxConcurrentDownloads = config.maxConcurrentDownloads;
  form.downloadLimitKb = bytesToKb(config.downloadLimit);
  form.uploadLimitKb = bytesToKb(config.uploadLimit);
  form.language = config.language;
}

function buildPayload(): AppConfig {
  return {
    defaultDownloadDir: form.defaultDownloadDir,
    maxConcurrentDownloads: Math.trunc(form.maxConcurrentDownloads || 1),
    downloadLimit: kbToBytes(form.downloadLimitKb),
    uploadLimit: kbToBytes(form.uploadLimitKb),
    language: form.language,
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

</script>

<template>
  <AppDialog
    :show="show"
    :title="t('settings.title')"
    width="620px"
    :mask-closable="!settingsStore.isSaving"
    :close-disabled="settingsStore.isSaving"
    @update:show="emit('update:show', $event)"
  >
    <NForm
        class="settings-form"
        :label-placement="isMobileLayout ? 'top' : 'left'"
        :label-width="isMobileLayout ? undefined : 150"
        :disabled="settingsStore.isLoading"
      >
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

        <WebAuthSettings />
        <JsonRpcTokenSettings :active="show" />
    </NForm>

    <template #footer>
      <AppDialogActions>
        <NButton :disabled="settingsStore.isSaving" @click="closeDialog">{{ t("common.cancel") }}</NButton>
        <NButton type="primary" :loading="settingsStore.isSaving" :disabled="!canSave" @click="saveSettings">
          {{ t("common.save") }}
        </NButton>
      </AppDialogActions>
    </template>
  </AppDialog>
</template>

<style scoped>
.setting-stack {
  width: 100%;
}

@media (max-width: 767px) {
  .settings-form :deep(.n-form-item-label) {
    padding-bottom: 8px;
  }

  .settings-form :deep(.n-form-item-blank),
  .settings-form :deep(.n-base-selection),
  .settings-form :deep(.n-input),
  .settings-form :deep(.n-input-number) {
    width: 100%;
  }

}
</style>
