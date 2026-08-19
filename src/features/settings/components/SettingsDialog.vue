<script setup lang="ts">
import { computed, reactive, ref, watch } from "vue";
import {
  NAlert,
  NButton,
  NForm,
  NFormItem,
  NInputNumber,
  NSelect,
  NTabPane,
  NTabs,
  NSpace,
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
import LanJsonRpcSettings from "./LanJsonRpcSettings.vue";
import ProxySettings from "./ProxySettings.vue";
import { useDownloadProxyStore } from "../stores/downloadProxyStore";
import AppIcon from "../../../components/AppIcon.vue";
import { fnosHost, type FnosHostKind, type SharedFolderAuthorizationResult } from "../../../services/fnos";

const props = defineProps<{
  show: boolean;
}>();

const emit = defineEmits<{
  "update:show": [value: boolean];
  openRpcGuide: [];
}>();

const message = useMessage();
const settingsStore = useSettingsStore();
const downloadProxyStore = useDownloadProxyStore();
const { t } = useI18n();
const { isMobileLayout } = useMobileLayout();
type SettingsSection = "preferences" | "proxy" | "security" | "rpc";
type RpcSection = "public" | "lan";

const activeSection = ref<SettingsSection>("preferences");
const activeRpcSection = ref<RpcSection>("public");
const form = reactive({
  defaultDownloadDir: "",
  maxConcurrentDownloads: 5,
  downloadLimitKb: 0,
  uploadLimitKb: 0,
  language: "zh-CN" as AppConfig["language"],
});
const hostKind = ref<FnosHostKind | null>(null);
const isDetectingHost = ref(false);
const isAuthorizing = ref(false);
const accessiblePathOptions = computed(() =>
  settingsStore.accessiblePaths.map((path) => ({
    label: settingsStore.displayAccessiblePaths.find((item) => item.path === path)?.displayPath || path,
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
    !downloadProxyStore.isSaving &&
    !settingsStore.isLoading &&
    !settingsStore.isLoadingAccessiblePaths &&
    !isDefaultDownloadDirUnauthorized.value,
);
const isSettingsSaving = computed(() => settingsStore.isSaving || downloadProxyStore.isSaving);
const hostSupportsAuthorization = computed(() => hostKind.value === "hosted" || hostKind.value === "mobile");
const accessiblePathsHelp = computed(() => {
  if (hostKind.value === null) return t("settings.accessiblePaths.detecting");
  return hostSupportsAuthorization.value
    ? t("settings.accessiblePaths.hostHelp")
    : t("settings.accessiblePaths.manualHelp");
});
const sectionOptions = computed(() => [
  { label: t("settings.sections.preferences"), value: "preferences" as SettingsSection },
  { label: t("settings.sections.proxy"), value: "proxy" as SettingsSection },
  { label: t("settings.sections.security"), value: "security" as SettingsSection },
  { label: t("settings.sections.rpc"), value: "rpc" as SettingsSection },
]);

watch(
  () => props.show,
  (show) => {
    if (show) {
      activeSection.value = "preferences";
      activeRpcSection.value = "public";
      void loadSettings();
    }
  },
  { immediate: true },
);

watch(
  () => form.language,
  (nextLanguage, previousLanguage) => {
    if (props.show && nextLanguage !== previousLanguage && settingsStore.accessiblePaths.length > 0) {
      void settingsStore.loadDisplayAccessiblePaths(nextLanguage);
    }
  },
);

async function loadSettings() {
  try {
    const [config] = await Promise.all([settingsStore.loadConfig(), settingsStore.loadAccessiblePaths(), detectHostKind()]);
    applyConfig(config);
  } catch (error) {
    message.error(getErrorMessage(error, t("settings.failed")));
  }
}

async function detectHostKind() {
  isDetectingHost.value = true;
  try {
    hostKind.value = await fnosHost.getHostKind();
  } finally {
    isDetectingHost.value = false;
  }
}

async function addAccessiblePath() {
  if (isAuthorizing.value || !hostSupportsAuthorization.value) {
    message.info(t("settings.accessiblePaths.manualHelp"));
    return;
  }

  isAuthorizing.value = true;
  try {
    const result = await fnosHost.requestSharedFolderAuthorization();
    await handleAuthorizationResult(result);
  } finally {
    isAuthorizing.value = false;
  }
}

async function handleAuthorizationResult(result: SharedFolderAuthorizationResult) {
  if (result.status === "cancelled") return;
  if (result.status === "admin_required") {
    message.error(t("settings.accessiblePaths.adminRequired"));
    return;
  }
  if (result.status === "unsupported") {
    message.info(t("settings.accessiblePaths.manualHelp"));
    return;
  }
  if (result.status === "failed") {
    message.error(t("settings.accessiblePaths.failed"));
    return;
  }

  try {
    await settingsStore.refreshAccessiblePaths();
    message.success(t("settings.accessiblePaths.authorized"));
  } catch (error) {
    message.warning(t("settings.accessiblePaths.stale"));
    message.error(getErrorMessage(error, t("settings.accessiblePaths.refreshFailed")));
  }
}

async function refreshAccessiblePathList() {
  if (settingsStore.isLoadingAccessiblePaths || isAuthorizing.value) return;
  try {
    await settingsStore.refreshAccessiblePaths();
    message.success(t("settings.accessiblePaths.authorized"));
  } catch (error) {
    message.warning(t("settings.accessiblePaths.stale"));
    message.error(getErrorMessage(error, t("settings.accessiblePaths.refreshFailed")));
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
    width="720px"
    fixed-body
    content-class="settings-dialog-content"
    :mask-closable="!isSettingsSaving"
    :close-disabled="isSettingsSaving"
    @update:show="emit('update:show', $event)"
  >
    <NForm
      class="settings-form"
      :label-placement="isMobileLayout ? 'top' : 'left'"
      :label-width="isMobileLayout ? undefined : 150"
      :disabled="settingsStore.isLoading"
    >
      <NSelect
        v-if="isMobileLayout"
        v-model:value="activeSection"
        class="settings-section-select"
        size="large"
        :options="sectionOptions"
        :aria-label="t('settings.navigation.label')"
      />

      <NTabs
        v-model:value="activeSection"
        class="settings-sections-tabs"
        type="line"
        pane-class="settings-pane"
        :aria-label="t('settings.navigation.label')"
      >
        <NTabPane name="preferences" :tab="t('settings.sections.preferences')" display-directive="show:lazy">
          <section class="settings-preferences">
            <header class="settings-section-heading">
              <div>
                <h3>{{ t("settings.sections.preferences") }}</h3>
                <p>{{ t("settings.sections.preferencesHelp") }}</p>
                <p class="settings-save-note">{{ t("settings.sections.preferencesSaveNote") }}</p>
              </div>
            </header>

            <div class="settings-preferences-fields">
              <div class="settings-accessible-paths" data-test="accessible-paths-settings">
                <div class="settings-accessible-paths-heading">
                  <div>
                    <h4>{{ t("settings.accessiblePaths.title") }}</h4>
                    <p>{{ t("settings.accessiblePaths.help") }}</p>
                  </div>
                  <span class="settings-accessible-path-count">{{ settingsStore.accessiblePaths.length }}</span>
                </div>
                <NAlert type="info" :bordered="false">{{ accessiblePathsHelp }}</NAlert>
                <NAlert v-if="settingsStore.accessiblePathsStale" type="warning" :bordered="false">
                  {{ t("settings.accessiblePaths.stale") }}
                </NAlert>
                <NSpace wrap>
                  <NButton
                    v-if="hostSupportsAuthorization"
                    type="primary"
                    :loading="isAuthorizing"
                    :disabled="isDetectingHost || isSettingsSaving"
                    @click="addAccessiblePath"
                  >
                    <template #icon><AppIcon name="plus" :size="16" /></template>
                    {{ t("settings.accessiblePaths.add") }}
                  </NButton>
                  <NButton
                    :loading="settingsStore.isLoadingAccessiblePaths"
                    :disabled="isAuthorizing || isSettingsSaving"
                    @click="refreshAccessiblePathList"
                  >
                    <template #icon><AppIcon name="refresh" :size="16" /></template>
                    {{ t("settings.accessiblePaths.refresh") }}
                  </NButton>
                </NSpace>
              </div>
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

              <NFormItem class="settings-form-note" :label="t('settings.background')">
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
            </div>
          </section>
        </NTabPane>

        <NTabPane name="proxy" :tab="t('settings.sections.proxy')" display-directive="show:lazy">
          <div class="settings-pane-section">
            <p class="settings-section-description">{{ t("settings.sections.proxyHelp") }}</p>
            <ProxySettings :active="show" />
          </div>
        </NTabPane>

        <NTabPane name="security" :tab="t('settings.sections.security')" display-directive="show:lazy">
          <div class="settings-pane-section">
            <p class="settings-section-description">{{ t("settings.sections.securityHelp") }}</p>
            <WebAuthSettings />
          </div>
        </NTabPane>

        <NTabPane name="rpc" :tab="t('settings.sections.rpc')" display-directive="show:lazy">
          <div class="settings-pane-section">
            <p class="settings-section-description">{{ t("settings.sections.rpcHelp") }}</p>
            <NTabs
              v-model:value="activeRpcSection"
              class="settings-rpc-tabs"
              type="line"
              placement="left"
              pane-class="settings-rpc-pane"
              :aria-label="t('settings.navigation.label')"
            >
              <NTabPane name="public" :tab="t('settings.rpcSections.public')" display-directive="show:lazy">
                <JsonRpcTokenSettings :active="show" @open-guide="emit('openRpcGuide')" />
              </NTabPane>
              <NTabPane name="lan" :tab="t('settings.rpcSections.lan')" display-directive="show:lazy">
                <LanJsonRpcSettings :active="show" @open-guide="emit('openRpcGuide')" />
              </NTabPane>
            </NTabs>
          </div>
        </NTabPane>
      </NTabs>
    </NForm>

    <template #footer>
      <AppDialogActions>
        <NButton :disabled="isSettingsSaving" @click="closeDialog">{{ t("common.cancel") }}</NButton>
        <NButton type="primary" :loading="settingsStore.isSaving" :disabled="!canSave" @click="saveSettings">
          {{ t("common.save") }}
        </NButton>
      </AppDialogActions>
    </template>
  </AppDialog>
</template>

<style scoped src="./SettingsDialog.css"></style>
