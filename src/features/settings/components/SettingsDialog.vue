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
  useMessage,
} from "naive-ui";
import AppDialog from "../../../components/ui/AppDialog.vue";
import AppDialogActions from "../../../components/ui/AppDialogActions.vue";
import { useSettingsStore } from "../stores/settingsStore";
import { useMobileLayout } from "../../../app/composables/useMobileLayout";
import { supportedLanguages, useI18n } from "../../../i18n";
import { getErrorMessage } from "../../../app/utils/errors";
import {
  DEFAULT_CONNECT_TIMEOUT,
  DEFAULT_MAX_CONNECTION_PER_SERVER,
  DEFAULT_MAX_CONCURRENT_DOWNLOADS,
  DEFAULT_MAX_TRIES,
  DEFAULT_MIN_SPLIT_SIZE,
  DEFAULT_SPLIT,
  MAX_CONNECT_TIMEOUT_LIMIT,
  MAX_CONNECTION_PER_SERVER_LIMIT,
  MAX_CONCURRENT_DOWNLOADS_LIMIT,
  MAX_SPLIT_LIMIT,
  MAX_TRIES_LIMIT,
  MIN_SPLIT_SIZE_OPTIONS,
  type AppConfig,
} from "../../../types/settings";
import WebAuthSettings from "../../auth/components/WebAuthSettings.vue";
import JsonRpcTokenSettings from "./JsonRpcTokenSettings.vue";
import LanJsonRpcSettings from "./LanJsonRpcSettings.vue";
import ProxySettings from "./ProxySettings.vue";
import { useDownloadProxyStore } from "../stores/downloadProxyStore";
import { useTaskStore } from "../../tasks/stores/taskStore";
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
const taskStore = useTaskStore();
const { language: currentLanguage, setLanguage, t } = useI18n();
const { isMobileLayout } = useMobileLayout();
type SettingsSection = "preferences" | "proxy" | "security" | "rpc";
type PreferencesSection = "authorization" | "interface" | "download";
type RpcSection = "public" | "lan";

const activeSection = ref<SettingsSection>("preferences");
const activePreferencesSection = ref<PreferencesSection>("authorization");
const activeRpcSection = ref<RpcSection>("public");
const savedLanguage = ref<AppConfig["language"] | null>(null);
const form = reactive({
  defaultDownloadDir: "",
  maxConcurrentDownloads: DEFAULT_MAX_CONCURRENT_DOWNLOADS,
  maxConnectionPerServer: DEFAULT_MAX_CONNECTION_PER_SERVER,
  split: DEFAULT_SPLIT,
  minSplitSize: DEFAULT_MIN_SPLIT_SIZE,
  connectTimeout: DEFAULT_CONNECT_TIMEOUT,
  maxTries: DEFAULT_MAX_TRIES,
  downloadLimitKb: 0,
  uploadLimitKb: 0,
  language: "zh-CN" as AppConfig["language"],
});
const hostKind = ref<FnosHostKind | null>(null);
const isDetectingHost = ref(false);
const isAuthorizing = ref(false);
const activeDownloadCount = computed(() => taskStore.tasks.filter((task) => task.status === "active").length);
const pendingDownloadCount = computed(() => taskStore.tasks.filter((task) => task.status === "pending").length);
const downloadSummary = computed(() =>
  t("settings.downloadSummary", {
    active: activeDownloadCount.value,
    pending: pendingDownloadCount.value,
    max: form.maxConcurrentDownloads,
  }),
);
const speedSummary = computed(() =>
  t("settings.speedSummary", {
    download: formatSpeed(form.downloadLimitKb),
    upload: formatSpeed(form.uploadLimitKb),
  }),
);
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
const minSplitSizeOptions = MIN_SPLIT_SIZE_OPTIONS.map((value) => ({ label: value, value }));
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
      activePreferencesSection.value = "authorization";
      activeRpcSection.value = "public";
      savedLanguage.value = currentLanguage.value;
      void loadSettings();
    } else {
      restoreSavedLanguage();
    }
  },
  { immediate: true },
);

watch(
  () => form.language,
  (nextLanguage, previousLanguage) => {
    if (props.show && nextLanguage !== previousLanguage) {
      setLanguage(nextLanguage);
      if (settingsStore.accessiblePaths.length > 0) {
        void settingsStore.loadDisplayAccessiblePaths(nextLanguage);
      }
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
    showSaveResult(config.runtimeApply);
    closeDialog();
  } catch (error) {
    message.error(getErrorMessage(error, t("settings.failed")));
  }
}

function showSaveResult(status: AppConfig["runtimeApply"]) {
  if (status === "applied") {
    message.success(t("settings.runtimeApply.applied"));
  } else if (status === "deferred") {
    message.info(t("settings.runtimeApply.deferred"));
  } else if (status === "failed") {
    message.warning(t("settings.runtimeApply.failed"));
  } else {
    message.success(t("settings.saved"));
  }
}

function applyConfig(config: AppConfig) {
  form.defaultDownloadDir = config.defaultDownloadDir;
  form.maxConcurrentDownloads = config.maxConcurrentDownloads;
  form.maxConnectionPerServer = config.maxConnectionPerServer ?? DEFAULT_MAX_CONNECTION_PER_SERVER;
  form.split = config.split ?? DEFAULT_SPLIT;
  form.minSplitSize = config.minSplitSize ?? DEFAULT_MIN_SPLIT_SIZE;
  form.connectTimeout = config.connectTimeout ?? DEFAULT_CONNECT_TIMEOUT;
  form.maxTries = config.maxTries ?? DEFAULT_MAX_TRIES;
  form.downloadLimitKb = bytesToKb(config.downloadLimit);
  form.uploadLimitKb = bytesToKb(config.uploadLimit);
  form.language = config.language;
  savedLanguage.value = config.language;
}

function buildPayload(): AppConfig {
  return {
    defaultDownloadDir: form.defaultDownloadDir,
    maxConcurrentDownloads: Math.trunc(form.maxConcurrentDownloads || 1),
    maxConnectionPerServer: Math.trunc(form.maxConnectionPerServer || 1),
    split: Math.trunc(form.split || 1),
    minSplitSize: form.minSplitSize,
    connectTimeout: Math.trunc(form.connectTimeout || 1),
    maxTries: Math.trunc(form.maxTries || 1),
    downloadLimit: kbToBytes(form.downloadLimitKb),
    uploadLimit: kbToBytes(form.uploadLimitKb),
    language: form.language,
  };
}

function closeDialog() {
  restoreSavedLanguage();
  emit("update:show", false);
}

function handleDialogVisibilityChange(nextShow: boolean) {
  if (!nextShow) restoreSavedLanguage();
  emit("update:show", nextShow);
}

function restoreSavedLanguage() {
  if (savedLanguage.value && currentLanguage.value !== savedLanguage.value) {
    setLanguage(savedLanguage.value);
  }
}

function bytesToKb(value: number) {
  return Math.floor(Math.max(0, value) / 1024);
}

function kbToBytes(value: number) {
  return Math.floor(Math.max(0, value || 0) * 1024);
}

function formatSpeed(value: number) {
  return value > 0 ? `${value} KB/s` : t("settings.unlimited");
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
    @update:show="handleDialogVisibilityChange"
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
          <section class="settings-preferences settings-preferences-section">
            <header class="settings-section-heading">
              <div>
                <p class="settings-save-note">{{ t("settings.sections.preferencesSaveNote") }}</p>
              </div>
            </header>

            <NTabs
              v-model:value="activePreferencesSection"
              class="settings-preferences-tabs"
              type="line"
              :placement="isMobileLayout ? 'top' : 'left'"
              pane-class="settings-preferences-pane"
              :aria-label="t('settings.preferenceTabs.label')"
            >
              <NTabPane
                name="authorization"
                :tab="t('settings.preferenceTabs.authorization')"
                display-directive="show:lazy"
              >
                <div class="settings-preferences-fields settings-authorization-fields">
                  <section class="settings-accessible-paths" data-test="accessible-paths-settings">
                    <div class="settings-accessible-paths-heading">
                      <div>
                        <span class="settings-accessible-paths-kicker">fnOS</span>
                        <h4>{{ t("settings.accessiblePaths.title") }}</h4>
                        <p>{{ t("settings.accessiblePaths.help") }}</p>
                      </div>
                      <span class="settings-accessible-path-count">{{ settingsStore.accessiblePaths.length }}</span>
                    </div>
                    <div class="settings-accessible-paths-status">
                      <NAlert type="info" :bordered="false">{{ accessiblePathsHelp }}</NAlert>
                      <NAlert v-if="settingsStore.accessiblePathsStale" type="warning" :bordered="false">
                        {{ t("settings.accessiblePaths.stale") }}
                      </NAlert>
                    </div>
                    <div class="settings-accessible-paths-actions">
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
                  </section>
                  <section class="settings-default-download-card">
                    <div class="settings-default-download-copy">
                      <span class="settings-default-download-kicker">{{ t("settings.preferenceTabs.download") }}</span>
                      <h4>{{ t("settings.defaultDownloadDir") }}</h4>
                      <p>{{ t("settings.defaultDownloadDir.help") }}</p>
                    </div>
                    <NFormItem
                      class="settings-default-download-field"
                      :label="t('settings.defaultDownloadDir')"
                      :feedback="defaultDownloadDirMessage"
                      :validation-status="isDefaultDownloadDirUnauthorized || settingsStore.accessiblePathsError ? 'warning' : undefined"
                    >
                      <NSelect
                        v-model:value="form.defaultDownloadDir"
                        :options="accessiblePathOptions"
                        :loading="settingsStore.isLoadingAccessiblePaths"
                        :placeholder="t('settings.defaultDownloadDir.placeholder')"
                        :aria-label="t('settings.defaultDownloadDir')"
                        filterable
                      />
                    </NFormItem>
                  </section>
                </div>
              </NTabPane>

              <NTabPane name="interface" :tab="t('settings.preferenceTabs.interface')" display-directive="show:lazy">
                <div class="settings-preferences-fields">
                  <NFormItem :label="t('settings.language')">
                    <NSelect v-model:value="form.language" :options="languageOptions" />
                  </NFormItem>
                </div>
              </NTabPane>

              <NTabPane name="download" :tab="t('settings.preferenceTabs.download')" display-directive="show:lazy">
                <div class="settings-preferences-fields settings-download-fields">
                  <NAlert type="warning" :bordered="false">{{ t("settings.maxConcurrentDownloadsHelp") }}</NAlert>
                  <NAlert type="info" :bordered="false">{{ t("settings.connectionTuningHelp") }}</NAlert>
                  <div class="settings-download-summary" data-test="download-settings-summary">
                    <p>{{ downloadSummary }}</p>
                    <p>{{ speedSummary }}</p>
                  </div>
                  <NFormItem :label="t('settings.maxConcurrentDownloads')">
                    <NInputNumber
                      v-model:value="form.maxConcurrentDownloads"
                      :min="1"
                      :max="MAX_CONCURRENT_DOWNLOADS_LIMIT"
                      :step="1"
                    />
                  </NFormItem>

                  <NFormItem :label="t('settings.maxConnectionPerServer')">
                    <NInputNumber
                      v-model:value="form.maxConnectionPerServer"
                      :min="1"
                      :max="MAX_CONNECTION_PER_SERVER_LIMIT"
                      :step="1"
                    />
                  </NFormItem>

                  <NFormItem :label="t('settings.defaultSplit')">
                    <NInputNumber v-model:value="form.split" :min="1" :max="MAX_SPLIT_LIMIT" :step="1" />
                  </NFormItem>

                  <NFormItem :label="t('settings.minSplitSize')">
                    <NSelect v-model:value="form.minSplitSize" :options="minSplitSizeOptions" />
                  </NFormItem>

                  <NFormItem :label="t('settings.connectTimeout')">
                    <NInputNumber
                      v-model:value="form.connectTimeout"
                      :min="1"
                      :max="MAX_CONNECT_TIMEOUT_LIMIT"
                      :step="1"
                    >
                      <template #suffix>s</template>
                    </NInputNumber>
                  </NFormItem>

                  <NFormItem :label="t('settings.maxTries')">
                    <NInputNumber v-model:value="form.maxTries" :min="1" :max="MAX_TRIES_LIMIT" :step="1" />
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
              </NTabPane>
            </NTabs>
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
          <div class="settings-pane-section settings-rpc-section">
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
