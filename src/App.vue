<script setup lang="ts">
import { onMounted, ref } from "vue";
import EngineStatusPanel from "./components/EngineStatusPanel.vue";
import { getAppInfo, pingBackend } from "./services/backend";
import type { AppInfo, BackendPing } from "./types/app";

const appInfo = ref<AppInfo | null>(null);
const backendPing = ref<BackendPing | null>(null);
const errorMessage = ref("");

async function refreshBackendStatus() {
  errorMessage.value = "";

  try {
    const [info, ping] = await Promise.all([getAppInfo(), pingBackend()]);
    appInfo.value = info;
    backendPing.value = ping;
  } catch (error) {
    errorMessage.value = error instanceof Error ? error.message : String(error);
  }
}

onMounted(() => {
  void refreshBackendStatus();
});
</script>

<template>
  <main class="app-shell">
    <section class="hero-card">
      <p class="eyebrow">阶段 0 / 工程骨架</p>
      <h1>{{ appInfo?.name ?? "Motrix FNOS" }}</h1>
      <p class="description">飞牛 OS GUI 下载工具骨架已启动，用于验证 Tauri 前后端通信。</p>

      <div class="status-grid">
        <div class="status-item">
          <span class="label">后端状态</span>
          <strong>{{ appInfo?.backendStatus ?? "checking" }}</strong>
        </div>
        <div class="status-item">
          <span class="label">应用版本</span>
          <strong>{{ appInfo?.version ?? "-" }}</strong>
        </div>
        <div class="status-item">
          <span class="label">Ping</span>
          <strong>{{ backendPing?.message ?? "等待响应" }}</strong>
        </div>
      </div>

      <p v-if="errorMessage" class="error-message">{{ errorMessage }}</p>
      <button type="button" @click="refreshBackendStatus">重新检查通信</button>
    </section>

    <EngineStatusPanel />
  </main>
</template>

<style scoped>
.app-shell {
  min-height: 100vh;
  display: grid;
  align-content: center;
  gap: 24px;
  padding: 32px;
  color: #e5f4ee;
  background: #0d1110;
}

.hero-card {
  width: min(1040px, 100%);
  padding: 32px;
  border: 1px solid rgba(113, 245, 177, 0.16);
  border-radius: 18px;
  background: linear-gradient(145deg, #151b1a, #101514);
  box-shadow: 0 18px 60px rgba(0, 0, 0, 0.36);
}

.eyebrow {
  margin: 0 0 10px;
  color: #67dca0;
  font-size: 13px;
  font-weight: 700;
  letter-spacing: 0.08em;
}

h1 {
  margin: 0;
  font-size: 40px;
  line-height: 1.1;
}

.description {
  margin: 16px 0 24px;
  color: #9fb3ab;
}

.status-grid {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 12px;
  margin-bottom: 24px;
}

.status-item {
  padding: 16px;
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: 14px;
  background: rgba(255, 255, 255, 0.04);
}

.label {
  display: block;
  margin-bottom: 8px;
  color: #7f918a;
  font-size: 12px;
}

strong {
  color: #ffffff;
}

.error-message {
  color: #ff8d8d;
}

button {
  border: 0;
  border-radius: 999px;
  padding: 11px 18px;
  color: #082014;
  background: #67dca0;
  font-weight: 700;
  cursor: pointer;
}

button:hover {
  background: #7af0b3;
}
</style>
