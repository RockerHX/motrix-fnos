<script setup lang="ts">
import { onMounted, ref } from "vue";
import {
  getAria2ConfigStatus,
  getAria2ProcessStatus,
  pingAria2Rpc,
  startAria2,
  stopAria2,
} from "../services/aria2";
import type { Aria2ConfigStatus, Aria2ProcessStatus, Aria2RpcStatus } from "../types/aria2";

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
  const [config, process] = await Promise.all([getAria2ConfigStatus(), getAria2ProcessStatus()]);
  configStatus.value = config;
  processStatus.value = process;
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
        <h2>引擎状态验证</h2>
      </div>
      <button type="button" class="ghost-button" :disabled="loading" @click="refreshEngineStatus">刷新</button>
    </div>

    <div class="engine-grid">
      <div class="engine-card">
        <span class="label">路径配置</span>
        <strong>{{ configStatus?.configured ? "已配置" : "未配置" }}</strong>
        <p>{{ configStatus?.path ?? "请设置 MOTRIX_FNOS_ARIA2_PATH" }}</p>
        <small>{{ configStatus?.pathExists ? "路径可用" : "路径未验证通过" }}</small>
      </div>

      <div class="engine-card">
        <span class="label">进程状态</span>
        <strong>{{ processStatus?.running ? "运行中" : "未运行" }}</strong>
        <p>{{ processStatus?.message ?? "等待检查" }}</p>
        <small>PID：{{ processStatus?.pid ?? "-" }}</small>
      </div>

      <div class="engine-card">
        <span class="label">RPC 状态</span>
        <strong>{{ rpcStatus?.connected ? "已连接" : "未连接" }}</strong>
        <p>{{ rpcStatus?.message ?? "尚未检查 RPC" }}</p>
        <small>版本：{{ rpcStatus?.version ?? "-" }}</small>
      </div>
    </div>

    <p v-if="errorMessage" class="error-message">{{ errorMessage }}</p>

    <div class="actions">
      <button type="button" :disabled="loading" @click="runAction(startAria2)">启动引擎</button>
      <button type="button" :disabled="loading" @click="runAction(stopAria2)">停止引擎</button>
      <button type="button" :disabled="loading" @click="runAction(pingAria2Rpc)">检查 RPC</button>
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
