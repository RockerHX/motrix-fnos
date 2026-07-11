<script setup lang="ts">
import { NButton } from "naive-ui";
import AppDialog from "../../../components/ui/AppDialog.vue";
import { ref, watch } from "vue";
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

    <div class="diagnostics-grid">
        <div><span>{{ t("diagnostics.appVersion") }}</span><strong>{{ props.appInfo?.version ?? "-" }}</strong></div>
        <div><span>{{ t("diagnostics.backendStatus") }}</span><strong>{{ props.appInfo?.backendStatus ?? t("diagnostics.backendChecking") }}</strong></div>
        <div><span>{{ t("diagnostics.communication") }}</span><strong>{{ props.backendPing?.message ?? t("common.loading") }}</strong></div>
        <div><span>{{ t("diagnostics.aria2Process") }}</span><strong>{{ props.aria2Process?.running ? t("diagnostics.running") : t("diagnostics.stopped") }}</strong></div>
        <div><span>{{ t("diagnostics.aria2Rpc") }}</span><strong>{{ props.aria2Rpc?.connected ? t("diagnostics.connected") : t("diagnostics.disconnected") }}</strong></div>
      </div>

    <EngineStatusPanel @status-updated="updateEngineStatus" />
  </AppDialog>

  <DebugLogDialog v-model:show="showDebugLogs" />
</template>

<style scoped>
h2 {
  margin: 0;
}

.diagnostics-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 12px;
  margin-bottom: 16px;
}

.diagnostics-grid div {
  padding: 14px;
  border-radius: var(--app-radius-sm);
  background: var(--app-color-card-overlay);
}

.diagnostics-grid span {
  display: block;
  margin-bottom: 8px;
  color: var(--app-text-dim);
}

@media (max-width: 767px) {
  .diagnostics-grid {
    grid-template-columns: minmax(0, 1fr);
    gap: 10px;
  }
}
</style>
