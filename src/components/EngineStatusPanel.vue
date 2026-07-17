<script setup lang="ts">
import { NButton } from "naive-ui";
import { computed, onMounted, ref } from "vue";
import AppMetricGrid from "./ui/AppMetricGrid.vue";
import type { AppMetricItem } from "./ui/appMetric";
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
const engineMetrics = computed<AppMetricItem[]>(() => [
  {
    label: t("engine.pathConfig"),
    value: configStatus.value?.binarySource === "sidecar" ? t("engine.sidecar") : t("engine.externalPath"),
    detail: configStatus.value?.path ?? configStatus.value?.sidecarName ?? "aria2-next",
    note: configStatus.value?.binarySource === "sidecar"
      ? configStatus.value?.targetTriple
      : configStatus.value?.pathExists
        ? t("engine.pathAvailable")
        : t("engine.pathInvalid"),
  },
  {
    label: t("engine.processStatus"),
    value: processStatus.value?.running ? t("diagnostics.running") : t("diagnostics.stopped"),
    detail: processStatus.value?.message ?? t("engine.waiting"),
    note: `${t("engine.pid")}：${processStatus.value?.pid ?? "-"} / ${processStatus.value?.binarySource ?? "-"}`,
    tone: processStatus.value?.running ? "success" : "default",
  },
  {
    label: t("engine.rpcStatus"),
    value: rpcStatus.value?.connected ? t("diagnostics.connected") : t("diagnostics.disconnected"),
    detail: rpcStatus.value?.message ?? t("engine.rpcUnchecked"),
    note: `${t("engine.version")}：${rpcStatus.value?.version ?? "-"}`,
    tone: rpcStatus.value?.connected ? "success" : "default",
  },
]);

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
      <NButton class="engine-action-button ghost-button" secondary :loading="loading" :disabled="loading" @click="refreshEngineStatus">
        {{ t("common.refresh") }}
      </NButton>
    </div>

    <AppMetricGrid class="engine-metrics" :items="engineMetrics" :desktop-columns="3" :mobile-columns="1" />

    <p v-if="errorMessage" class="error-message">{{ errorMessage }}</p>

    <div class="actions">
      <NButton class="engine-action-button" type="primary" :loading="loading" :disabled="loading" @click="runAction(startAria2)">
        {{ t("engine.start") }}
      </NButton>
      <NButton class="engine-action-button" secondary :loading="loading" :disabled="loading" @click="runAction(stopAria2)">
        {{ t("engine.stop") }}
      </NButton>
      <NButton class="engine-action-button" secondary :loading="loading" :disabled="loading" @click="runAction(pingAria2Rpc)">
        {{ t("engine.checkRpc") }}
      </NButton>
    </div>
  </section>
</template>

<style scoped src="./EngineStatusPanel.css"></style>
