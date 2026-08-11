<script setup lang="ts">
import { NButton, NSpace } from "naive-ui";
import AppDialog from "../../../components/ui/AppDialog.vue";
import AppMetricGrid from "../../../components/ui/AppMetricGrid.vue";
import type { AppMetricItem } from "../../../components/ui/appMetric";
import { computed, ref, watch } from "vue";
import EngineStatusPanel from "../../../components/EngineStatusPanel.vue";
import Aria2LogModePanel from "./Aria2LogModePanel.vue";
import DebugLogDialog from "./DebugLogDialog.vue";
import { useI18n } from "../../../i18n";
import type { AppInfo, BackendPing } from "../../../types/app";
import type { Aria2ProcessStatus, Aria2RpcStatus } from "../../../types/aria2";
import { useLanJsonRpcStore } from "../../settings/stores/lanJsonRpcStore";
import { lanJsonRpcEndpoint } from "../../settings/utils/lanJsonRpcEndpoint";

type EngineStatusSnapshot = {
  process: Aria2ProcessStatus;
  rpc: Aria2RpcStatus;
};

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
const showDebugLogs = ref(false);
const lanEndpoint = computed(() => lanJsonRpcEndpoint(window.location.hostname));
const diagnosticMetrics = computed<AppMetricItem[]>(() => [
  { label: t("diagnostics.appVersion"), value: props.appInfo?.version ?? "-" },
  { label: t("diagnostics.backendStatus"), value: props.appInfo?.backendStatus ?? t("diagnostics.backendChecking") },
  { label: t("diagnostics.communication"), value: props.backendPing?.message ?? t("common.loading") },
  { label: t("diagnostics.aria2Process"), value: props.aria2Process?.running ? t("diagnostics.running") : t("diagnostics.stopped") },
  { label: t("diagnostics.aria2Rpc"), value: props.aria2Rpc?.connected ? t("diagnostics.connected") : t("diagnostics.disconnected") },
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

function updateLogMode() {
  emit("refreshStatus");
}
</script>

<template>
  <AppDialog
    :show="props.show"
    :eyebrow="t('diagnostics.eyebrow')"
    :title="t('diagnostics.title')"
    width="900px"
    @update:show="updateShow"
  >
    <template #header-extra>
      <NSpace>
        <NButton secondary @click="emit('openRpcGuide')">{{ t("diagnostics.jsonRpcGuide") }}</NButton>
        <NButton secondary @click="showDebugLogs = true">{{ t("diagnostics.debugLogs") }}</NButton>
      </NSpace>
    </template>

    <AppMetricGrid class="diagnostics-metrics" :items="diagnosticMetrics" :desktop-columns="2" :mobile-columns="1" />

    <Aria2LogModePanel :active="props.show" @updated="updateLogMode" />

    <EngineStatusPanel @status-updated="updateEngineStatus" />
  </AppDialog>

  <DebugLogDialog v-model:show="showDebugLogs" />
</template>

<style scoped src="./DiagnosticsDialog.css"></style>
