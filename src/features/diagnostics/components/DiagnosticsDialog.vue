<script setup lang="ts">
import { NButton, NCard, NModal } from "naive-ui";
import { ref } from "vue";
import EngineStatusPanel from "../../../components/EngineStatusPanel.vue";
import DebugLogDialog from "./DebugLogDialog.vue";
import type { AppInfo, BackendPing } from "../../../types/app";
import type { Aria2ProcessStatus, Aria2RpcStatus } from "../../../types/aria2";

defineProps<{
  show: boolean;
  appInfo: AppInfo | null;
  backendPing: BackendPing | null;
  aria2Process: Aria2ProcessStatus | null;
  aria2Rpc: Aria2RpcStatus | null;
}>();

const emit = defineEmits<{
  "update:show": [show: boolean];
}>();

const showDebugLogs = ref(false);

function updateShow(show: boolean) {
  emit("update:show", show);
}

function closeDialog() {
  updateShow(false);
}
</script>

<template>
  <NModal :show="show" @update:show="updateShow">
    <NCard class="diagnostics-dialog" role="dialog" aria-modal="true">
      <template #header>
        <div>
          <p class="eyebrow">Diagnostics</p>
          <h2>阶段状态与引擎诊断</h2>
        </div>
      </template>
      <template #header-extra>
        <div class="header-actions">
          <NButton secondary @click="showDebugLogs = true">调试日志</NButton>
          <NButton quaternary circle @click="closeDialog">×</NButton>
        </div>
      </template>

      <div class="diagnostics-grid">
        <div><span>应用版本</span><strong>{{ appInfo?.version ?? "-" }}</strong></div>
        <div><span>后端状态</span><strong>{{ appInfo?.backendStatus ?? "checking" }}</strong></div>
        <div><span>通信结果</span><strong>{{ backendPing?.message ?? "等待响应" }}</strong></div>
        <div><span>Aria2 进程</span><strong>{{ aria2Process?.running ? "运行中" : "未运行" }}</strong></div>
        <div><span>Aria2 RPC</span><strong>{{ aria2Rpc?.connected ? "已连接" : "未连接" }}</strong></div>
      </div>

      <EngineStatusPanel />
    </NCard>
  </NModal>

  <DebugLogDialog v-model:show="showDebugLogs" />
</template>

<style scoped>
.diagnostics-dialog {
  width: min(900px, calc(100vw - 48px));
  max-height: calc(100vh - 48px);
  overflow: auto;
}

.eyebrow {
  margin: 0 0 6px;
  color: #66e39a;
  font-size: 12px;
  font-weight: 800;
  letter-spacing: 0.1em;
  text-transform: uppercase;
}

h2 {
  margin: 0;
}

.header-actions {
  display: flex;
  align-items: center;
  gap: 8px;
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
</style>
