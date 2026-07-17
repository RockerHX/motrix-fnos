<script setup lang="ts">
import { NButton } from "naive-ui";
import AppDialog from "../../../components/ui/AppDialog.vue";
import AppMetricGrid from "../../../components/ui/AppMetricGrid.vue";
import { computed, ref, watch } from "vue";
import EngineStatusPanel from "../../../components/EngineStatusPanel.vue";
import DebugLogDialog from "./DebugLogDialog.vue";
import { useI18n } from "../../../i18n";
import type { AppInfo, BackendPing } from "../../../types/app";
import type { Aria2ProcessStatus, Aria2RpcStatus } from "../../../types/aria2";

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
}>();

const emit = defineEmits<{
  "update:show": [show: boolean];
  refreshStatus: [];
  engineStatusUpdated: [status: EngineStatusSnapshot];
}>();

const { t } = useI18n();
const showDebugLogs = ref(false);
const diagnosticMetrics = computed(() => [
  { label: t("diagnostics.appVersion"), value: props.appInfo?.version ?? "-" },
  { label: t("diagnostics.backendStatus"), value: props.appInfo?.backendStatus ?? t("diagnostics.backendChecking") },
  { label: t("diagnostics.communication"), value: props.backendPing?.message ?? t("common.loading") },
  { label: t("diagnostics.aria2Process"), value: props.aria2Process?.running ? t("diagnostics.running") : t("diagnostics.stopped") },
  { label: t("diagnostics.aria2Rpc"), value: props.aria2Rpc?.connected ? t("diagnostics.connected") : t("diagnostics.disconnected") },
]);

watch(
  () => props.show,
  (show) => {
    if (show) {
      emit("refreshStatus");
    }
  },
);

function updateShow(show: boolean) {
  emit("update:show", show);
}

function updateEngineStatus(status: EngineStatusSnapshot) {
  emit("engineStatusUpdated", status);
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
      <NButton secondary @click="showDebugLogs = true">{{ t("diagnostics.debugLogs") }}</NButton>
    </template>

    <AppMetricGrid class="diagnostics-metrics" :items="diagnosticMetrics" :desktop-columns="2" :mobile-columns="1" />

    <EngineStatusPanel @status-updated="updateEngineStatus" />
  </AppDialog>

  <DebugLogDialog v-model:show="showDebugLogs" />
</template>

<style scoped src="./DiagnosticsDialog.css"></style>
