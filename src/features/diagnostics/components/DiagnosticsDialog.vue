<script setup lang="ts">
import { NButton, NTabPane, NTabs, NSpace } from "naive-ui";
import AppDialog from "../../../components/ui/AppDialog.vue";
import AppIcon from "../../../components/AppIcon.vue";
import AppMetricGrid from "../../../components/ui/AppMetricGrid.vue";
import type { AppMetricItem } from "../../../components/ui/appMetric";
import { computed, ref, watch } from "vue";
import EngineStatusPanel from "../../../components/EngineStatusPanel.vue";
import Aria2LogModePanel from "./Aria2LogModePanel.vue";
import DebugLogDialog from "./DebugLogDialog.vue";
import LogMaintenancePanel from "./LogMaintenancePanel.vue";
import { useI18n } from "../../../i18n";
import type { AppInfo, BackendPing } from "../../../types/app";
import type { Aria2ProcessStatus, Aria2RpcStatus } from "../../../types/aria2";
import { useLanJsonRpcStore } from "../../settings/stores/lanJsonRpcStore";
import { lanJsonRpcEndpoint } from "../../settings/utils/lanJsonRpcEndpoint";
import { useDiagnosticBundleExport } from "../composables/useDiagnosticBundleExport";

type EngineStatusSnapshot = {
  process: Aria2ProcessStatus;
  rpc: Aria2RpcStatus;
};

type DiagnosticSection = "overview" | "connection" | "logs";

const props = defineProps<{
  show: boolean;
  appInfo: AppInfo | null;
  backendPing: BackendPing | null;
  aria2Process: Aria2ProcessStatus | null;
  aria2Rpc: Aria2RpcStatus | null;
  jsonRpcTokenConfigured: boolean | null;
}>();

const emit = defineEmits<{
  "update:show": [show: boolean];
  refreshStatus: [];
  engineStatusUpdated: [status: EngineStatusSnapshot];
  openRpcGuide: [];
}>();

const { t } = useI18n();
const lanJsonRpcStore = useLanJsonRpcStore();
const activeSection = ref<DiagnosticSection>("overview");
const showDebugLogs = ref(false);
const logMaintenanceRef = ref<InstanceType<typeof LogMaintenancePanel> | null>(null);
const { isExporting, exportDiagnosticBundle } = useDiagnosticBundleExport();
const lanEndpoint = computed(() => lanJsonRpcEndpoint(window.location.hostname));
const overviewMetrics = computed<AppMetricItem[]>(() => [
  { label: t("diagnostics.appVersion"), value: props.appInfo?.version ?? "-" },
  { label: t("diagnostics.backendStatus"), value: props.appInfo?.backendStatus ?? t("diagnostics.backendChecking") },
  { label: t("diagnostics.communication"), value: props.backendPing?.message ?? t("common.loading") },
]);
const connectionMetrics = computed<AppMetricItem[]>(() => [
  {
    label: t("diagnostics.jsonRpcEndpoint"),
    value: "127.0.0.1:17081",
    detail: "/jsonrpc",
    note: t("diagnostics.jsonRpcLoopback"),
  },
  {
    label: t("diagnostics.lanJsonRpcEndpoint"),
    value: lanEndpoint.value.value,
    detail: "/jsonrpc",
    note: t("diagnostics.lanJsonRpcSource"),
  },
  {
    label: t("diagnostics.jsonRpcToken"),
    value:
      props.jsonRpcTokenConfigured === true
        ? t("diagnostics.jsonRpcTokenConfigured")
        : props.jsonRpcTokenConfigured === false
          ? t("diagnostics.jsonRpcTokenMissing")
          : t("diagnostics.jsonRpcTokenUnknown"),
    note: t("diagnostics.jsonRpcTokenNote"),
    tone: props.jsonRpcTokenConfigured === true ? "success" : props.jsonRpcTokenConfigured === false ? "warning" : "default",
  },
  {
    label: t("diagnostics.lanJsonRpcToken"),
    value: lanJsonRpcStore.status?.enabled
      ? t("diagnostics.lanJsonRpcEnabled")
      : t("diagnostics.lanJsonRpcDisabled"),
    detail:
      lanJsonRpcStore.status?.configured === true
        ? t("diagnostics.jsonRpcTokenConfigured")
        : lanJsonRpcStore.status?.configured === false
          ? t("diagnostics.jsonRpcTokenMissing")
          : t("diagnostics.jsonRpcTokenUnknown"),
    note: t("diagnostics.jsonRpcTokenNote"),
    tone:
      lanJsonRpcStore.status?.enabled && lanJsonRpcStore.status?.configured
        ? "success"
        : lanJsonRpcStore.status
          ? "warning"
          : "default",
  },
]);

watch(
  () => props.show,
  (show) => {
    if (show) {
      activeSection.value = "overview";
      emit("refreshStatus");
      void lanJsonRpcStore.loadStatus().catch(() => undefined);
    }
  },
);

function updateShow(show: boolean) {
  emit("update:show", show);
}

function updateEngineStatus(status: EngineStatusSnapshot) {
  emit("engineStatusUpdated", status);
}

async function refreshLogUsage() {
  const panel = logMaintenanceRef.value as { refresh?: () => Promise<void> } | null;
  await panel?.refresh?.();
}

async function exportBundleAndRefresh() {
  await exportDiagnosticBundle();
  await refreshLogUsage();
}

function updateLogMode() {
  emit("refreshStatus");
  void refreshLogUsage();
}

function updateLogUsage() {
  emit("refreshStatus");
}
</script>

<template>
  <AppDialog
    :show="props.show"
    :eyebrow="t('diagnostics.eyebrow')"
    :title="t('diagnostics.title')"
    width="900px"
    fixed-body
    content-class="diagnostics-dialog-content"
    @update:show="updateShow"
  >
    <template #header-extra>
      <NSpace>
        <NButton secondary :loading="isExporting" @click="exportBundleAndRefresh">
          <template #icon><AppIcon name="download" :size="16" /></template>
          {{ t("diagnostics.bundle.export") }}
        </NButton>
      </NSpace>
    </template>

    <NTabs
      v-model:value="activeSection"
      class="diagnostics-tabs"
      type="line"
      pane-class="diagnostics-pane"
      :aria-label="t('diagnostics.navigation.label')"
    >
      <NTabPane name="overview" :tab="t('diagnostics.sections.overview')" display-directive="show:lazy">
        <p class="diagnostics-section-description">{{ t("diagnostics.sections.overviewHelp") }}</p>
        <AppMetricGrid class="diagnostics-metrics" :items="overviewMetrics" :desktop-columns="3" :mobile-columns="1" />
        <EngineStatusPanel @status-updated="updateEngineStatus" />
      </NTabPane>

      <NTabPane name="connection" :tab="t('diagnostics.sections.connection')" display-directive="show:lazy">
        <p class="diagnostics-section-description">{{ t("diagnostics.sections.connectionHelp") }}</p>
        <AppMetricGrid class="diagnostics-metrics" :items="connectionMetrics" :desktop-columns="2" :mobile-columns="1" />
        <div class="diagnostics-pane-actions">
          <NButton secondary @click="emit('openRpcGuide')">{{ t("diagnostics.jsonRpcGuide") }}</NButton>
        </div>
      </NTabPane>

      <NTabPane name="logs" :tab="t('diagnostics.sections.logs')" display-directive="show:lazy">
        <p class="diagnostics-section-description">{{ t("diagnostics.sections.logsHelp") }}</p>
        <Aria2LogModePanel :active="props.show" @updated="updateLogMode" />
        <LogMaintenancePanel
          ref="logMaintenanceRef"
          :active="props.show"
          :aria2-running="props.aria2Process?.running ?? null"
          @updated="updateLogUsage"
        />
        <div class="diagnostics-pane-actions">
          <NButton secondary @click="showDebugLogs = true">{{ t("diagnostics.debugLogs") }}</NButton>
        </div>
      </NTabPane>
    </NTabs>
  </AppDialog>

  <DebugLogDialog v-model:show="showDebugLogs" />
</template>

<style scoped src="./DiagnosticsDialog.css"></style>
