<script setup lang="ts">
import { NButton, NCard, NModal } from "naive-ui";
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

function closeDialog() {
  updateShow(false);
}

function updateEngineStatus(status: EngineStatusSnapshot) {
  emit("engineStatusUpdated", status);
}
</script>

<template>
  <NModal :show="props.show" @update:show="updateShow">
    <NCard class="diagnostics-dialog app-dialog" role="dialog" aria-modal="true">
      <template #header>
        <div>
          <p class="app-dialog-eyebrow">{{ t("diagnostics.eyebrow") }}</p>
          <h2>{{ t("diagnostics.title") }}</h2>
        </div>
      </template>
      <template #header-extra>
        <div class="app-dialog-header-actions">
          <NButton secondary @click="showDebugLogs = true">{{ t("diagnostics.debugLogs") }}</NButton>
          <NButton quaternary circle @click="closeDialog">×</NButton>
        </div>
      </template>

      <div class="diagnostics-grid">
        <div><span>{{ t("diagnostics.appVersion") }}</span><strong>{{ props.appInfo?.version ?? "-" }}</strong></div>
        <div><span>{{ t("diagnostics.backendStatus") }}</span><strong>{{ props.appInfo?.backendStatus ?? t("diagnostics.backendChecking") }}</strong></div>
        <div><span>{{ t("diagnostics.communication") }}</span><strong>{{ props.backendPing?.message ?? t("common.loading") }}</strong></div>
        <div><span>{{ t("diagnostics.aria2Process") }}</span><strong>{{ props.aria2Process?.running ? t("diagnostics.running") : t("diagnostics.stopped") }}</strong></div>
        <div><span>{{ t("diagnostics.aria2Rpc") }}</span><strong>{{ props.aria2Rpc?.connected ? t("diagnostics.connected") : t("diagnostics.disconnected") }}</strong></div>
      </div>

      <EngineStatusPanel @status-updated="updateEngineStatus" />
    </NCard>
  </NModal>

  <DebugLogDialog v-model:show="showDebugLogs" />
</template>

<style scoped>
.diagnostics-dialog {
  --app-dialog-width: 900px;
}

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
  border-radius: 12px;
  background: rgba(255, 255, 255, 0.05);
}

.diagnostics-grid span {
  display: block;
  margin-bottom: 8px;
  color: #84968f;
}

@media (max-width: 767px) {
  .diagnostics-grid {
    grid-template-columns: minmax(0, 1fr);
    gap: 10px;
  }
}
</style>
