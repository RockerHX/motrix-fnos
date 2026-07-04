<script setup lang="ts">
import { onMounted, ref } from "vue";
import {
  getAria2ConfigStatus,
  getAria2ProcessStatus,
  pingAria2Rpc,
  startAria2,
  stopAria2,
} from "../services/aria2";
import { useI18n } from "../i18n";
import type { Aria2ConfigStatus, Aria2ProcessStatus, Aria2RpcStatus } from "../types/aria2";

type EngineStatusSnapshot = {
  process: Aria2ProcessStatus;
  rpc: Aria2RpcStatus;
};

const emit = defineEmits<{
  statusUpdated: [status: EngineStatusSnapshot];
}>();

const { t } = useI18n();
const configStatus = ref<Aria2ConfigStatus | null>(null);
const processStatus = ref<Aria2ProcessStatus | null>(null);
const rpcStatus = ref<Aria2RpcStatus | null>(null);
const errorMessage = ref("");
const loading = ref(false);

defineExpose({
  refreshEngineStatus,
});

async function refreshEngineStatus() {
  errorMessage.value = "";
  const [config, process, rpc] = await Promise.all([
    getAria2ConfigStatus(),
    getAria2ProcessStatus(),
    pingAria2Rpc(),
  ]);
  configStatus.value = config;
  processStatus.value = process;
  rpcStatus.value = rpc;
  emit("statusUpdated", { process, rpc });
}

async function runAction(action: () => Promise<Aria2ProcessStatus | Aria2RpcStatus>) {
  loading.value = true;
  errorMessage.value = "";

  try {
    const result = await action();
    if ("running" in result) {
      processStatus.value = result;
    } else {
      rpcStatus.value = result;
    }
    await refreshEngineStatus();
  } catch (error) {
    errorMessage.value = error instanceof Error ? error.message : String(error);
  } finally {
    loading.value = false;
  }
}

onMounted(() => {
  void refreshEngineStatus();
});
</script>

<template>
  <section class="engine-panel">
    <div class="panel-header">
      <div>
        <p class="eyebrow">Aria2 Next</p>
        <h2>{{ t("engine.title") }}</h2>
      </div>
      <button type="button" class="ghost-button" :disabled="loading" @click="refreshEngineStatus">{{ t("common.refresh") }}</button>
    </div>

    <div class="engine-grid">
      <div class="engine-card">
        <span class="label">{{ t("engine.pathConfig") }}</span>
        <strong>{{ configStatus?.binarySource === "sidecar" ? t("engine.sidecar") : t("engine.externalPath") }}</strong>
        <p>{{ configStatus?.path ?? configStatus?.sidecarName ?? "aria2-next" }}</p>
        <small>{{ configStatus?.binarySource === "sidecar" ? configStatus?.targetTriple : configStatus?.pathExists ? t("engine.pathAvailable") : t("engine.pathInvalid") }}</small>
      </div>

      <div class="engine-card">
        <span class="label">{{ t("engine.processStatus") }}</span>
        <strong>{{ processStatus?.running ? t("diagnostics.running") : t("diagnostics.stopped") }}</strong>
        <p>{{ processStatus?.message ?? t("engine.waiting") }}</p>
        <small>{{ t("engine.pid") }}：{{ processStatus?.pid ?? "-" }} / {{ processStatus?.binarySource ?? "-" }}</small>
      </div>

      <div class="engine-card">
        <span class="label">{{ t("engine.rpcStatus") }}</span>
        <strong>{{ rpcStatus?.connected ? t("diagnostics.connected") : t("diagnostics.disconnected") }}</strong>
        <p>{{ rpcStatus?.message ?? t("engine.rpcUnchecked") }}</p>
        <small>{{ t("engine.version") }}：{{ rpcStatus?.version ?? "-" }}</small>
      </div>
    </div>

    <p v-if="errorMessage" class="error-message">{{ errorMessage }}</p>

    <div class="actions">
      <button type="button" :disabled="loading" @click="runAction(startAria2)">{{ t("engine.start") }}</button>
      <button type="button" :disabled="loading" @click="runAction(stopAria2)">{{ t("engine.stop") }}</button>
      <button type="button" :disabled="loading" @click="runAction(pingAria2Rpc)">{{ t("engine.checkRpc") }}</button>
    </div>
  </section>
</template>

<style scoped>
.engine-panel {
  padding: 24px;
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: 18px;
  background: #151b1a;
}

.panel-header,
.actions {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.eyebrow {
  margin: 0 0 6px;
  color: #67dca0;
  font-size: 12px;
  font-weight: 700;
  letter-spacing: 0.08em;
  text-transform: uppercase;
}

h2 {
  margin: 0;
  font-size: 20px;
}

.engine-grid {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 12px;
  margin: 20px 0;
}

.engine-card {
  min-width: 0;
  padding: 16px;
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: 14px;
  background: rgba(255, 255, 255, 0.04);
}

.label,
small {
  color: #83958e;
  font-size: 12px;
}

strong {
  display: block;
  margin: 8px 0;
  color: #ffffff;
}

p {
  overflow: hidden;
  margin: 0 0 8px;
  color: #a8bab3;
  text-overflow: ellipsis;
  white-space: nowrap;
}

button {
  border: 0;
  border-radius: 999px;
  padding: 10px 14px;
  color: #082014;
  background: #67dca0;
  font-weight: 700;
  cursor: pointer;
}

button:disabled {
  cursor: not-allowed;
  opacity: 0.55;
}

.ghost-button {
  color: #d7eee4;
  background: rgba(255, 255, 255, 0.08);
}

.actions {
  justify-content: flex-start;
}

.error-message {
  margin-bottom: 16px;
  color: #ff8d8d;
  white-space: normal;
}
</style>
