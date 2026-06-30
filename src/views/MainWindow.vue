<script setup lang="ts">
import EngineStatusPanel from "../components/EngineStatusPanel.vue";
import type { AppInfo, BackendPing } from "../types/app";

defineProps<{
  appInfo: AppInfo | null;
  backendPing: BackendPing | null;
  errorMessage: string;
}>();

const categories = [
  { name: "全部", count: 0 },
  { name: "下载中", count: 0 },
  { name: "已完成", count: 0 },
  { name: "做种", count: 0 },
  { name: "活动", count: 0 },
  { name: "暂停", count: 0 },
  { name: "错误", count: 0 },
];
</script>

<template>
  <div class="window-shell">
    <aside class="sidebar">
      <div class="brand">
        <div class="brand-mark">M</div>
        <div>
          <strong>{{ appInfo?.name ?? "Motrix FNOS" }}</strong>
          <span>飞牛下载工具</span>
        </div>
      </div>

      <nav class="category-list" aria-label="任务分类">
        <button v-for="category in categories" :key="category.name" type="button" :class="{ active: category.name === '全部' }">
          <span>{{ category.name }}</span>
          <em>{{ category.count }}</em>
        </button>
      </nav>
    </aside>

    <section class="main-area">
      <header class="toolbar">
        <div>
          <p class="eyebrow">阶段 0 / GUI 骨架</p>
          <h1>全部任务</h1>
        </div>
        <div class="toolbar-actions">
          <button type="button" class="primary">添加任务</button>
          <button type="button" disabled>开始</button>
          <button type="button" disabled>暂停</button>
          <button type="button" disabled>删除</button>
          <input type="search" placeholder="搜索任务" disabled />
        </div>
      </header>

      <main class="content-stack">
        <section class="empty-state">
          <div class="empty-icon">↓</div>
          <h2>暂无任务</h2>
          <p>阶段 0 只验证工程骨架、前后端通信和 Aria2 Next 引擎连接，不创建真实下载任务。</p>
          <button type="button" class="primary">添加第一个任务</button>
        </section>

        <section class="phase-status">
          <div class="phase-header">
            <div>
              <p class="eyebrow">Status Check</p>
              <h2>阶段 0 状态检查</h2>
            </div>
            <span :class="['status-pill', backendPing?.ok ? 'ok' : 'pending']">
              {{ backendPing?.ok ? "后端已连接" : "等待后端" }}
            </span>
          </div>

          <div class="status-grid">
            <div class="status-card">
              <span>应用版本</span>
              <strong>{{ appInfo?.version ?? "-" }}</strong>
            </div>
            <div class="status-card">
              <span>后端状态</span>
              <strong>{{ appInfo?.backendStatus ?? "checking" }}</strong>
            </div>
            <div class="status-card">
              <span>通信结果</span>
              <strong>{{ backendPing?.message ?? "等待响应" }}</strong>
            </div>
          </div>

          <p v-if="errorMessage" class="error-message">{{ errorMessage }}</p>
        </section>

        <EngineStatusPanel />
      </main>

      <footer class="status-bar">
        <span>下载 0 B/s</span>
        <span>上传 0 B/s</span>
        <span>连接数 0</span>
        <span>磁盘状态：待接入</span>
      </footer>
    </section>
  </div>
</template>

<style scoped>
.window-shell {
  min-height: 100vh;
  display: grid;
  grid-template-columns: 240px minmax(0, 1fr);
  color: #dce8e2;
  background: #0b0f0e;
}

.sidebar {
  padding: 22px 16px;
  border-right: 1px solid rgba(255, 255, 255, 0.08);
  background: #0f1514;
}

.brand {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-bottom: 28px;
}

.brand-mark {
  width: 38px;
  height: 38px;
  display: grid;
  place-items: center;
  border-radius: 12px;
  color: #062015;
  background: #67dca0;
  font-weight: 900;
}

.brand strong,
.brand span {
  display: block;
}

.brand span {
  margin-top: 2px;
  color: #7f918a;
  font-size: 12px;
}

.category-list {
  display: grid;
  gap: 6px;
}

.category-list button {
  display: flex;
  align-items: center;
  justify-content: space-between;
  border: 0;
  border-radius: 12px;
  padding: 11px 12px;
  color: #a9bbb4;
  background: transparent;
  text-align: left;
  cursor: pointer;
}

.category-list button.active,
.category-list button:hover {
  color: #ffffff;
  background: rgba(103, 220, 160, 0.12);
}

.category-list em {
  min-width: 24px;
  border-radius: 999px;
  color: #80a494;
  background: rgba(255, 255, 255, 0.06);
  font-style: normal;
  font-size: 12px;
  text-align: center;
}

.main-area {
  min-width: 0;
  display: grid;
  grid-template-rows: auto minmax(0, 1fr) auto;
}

.toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 20px;
  padding: 20px 24px;
  border-bottom: 1px solid rgba(255, 255, 255, 0.08);
  background: rgba(15, 21, 20, 0.94);
}

.eyebrow {
  margin: 0 0 5px;
  color: #67dca0;
  font-size: 12px;
  font-weight: 800;
  letter-spacing: 0.08em;
  text-transform: uppercase;
}

h1,
h2 {
  margin: 0;
  color: #ffffff;
}

.toolbar-actions {
  display: flex;
  align-items: center;
  gap: 8px;
}

button,
input {
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 999px;
  padding: 9px 13px;
  color: #dce8e2;
  background: rgba(255, 255, 255, 0.06);
  font: inherit;
}

button {
  cursor: pointer;
}

button:disabled,
input:disabled {
  cursor: not-allowed;
  opacity: 0.48;
}

.primary {
  border-color: transparent;
  color: #062015;
  background: #67dca0;
  font-weight: 800;
}

.content-stack {
  min-height: 0;
  overflow: auto;
  display: grid;
  gap: 18px;
  padding: 24px;
}

.empty-state,
.phase-status {
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: 18px;
  background: #151b1a;
}

.empty-state {
  min-height: 280px;
  display: grid;
  justify-items: center;
  align-content: center;
  padding: 32px;
  text-align: center;
}

.empty-icon {
  width: 72px;
  height: 72px;
  display: grid;
  place-items: center;
  margin-bottom: 18px;
  border-radius: 24px;
  color: #67dca0;
  background: rgba(103, 220, 160, 0.12);
  font-size: 42px;
}

.empty-state p {
  max-width: 520px;
  color: #8fa29a;
}

.phase-status {
  padding: 22px;
}

.phase-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  margin-bottom: 18px;
}

.status-pill {
  border-radius: 999px;
  padding: 7px 10px;
  color: #cbd8d2;
  background: rgba(255, 255, 255, 0.08);
  font-size: 12px;
  font-weight: 800;
}

.status-pill.ok {
  color: #082014;
  background: #67dca0;
}

.status-grid {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 12px;
}

.status-card {
  padding: 15px;
  border-radius: 14px;
  background: rgba(255, 255, 255, 0.05);
}

.status-card span {
  display: block;
  margin-bottom: 8px;
  color: #84968f;
  font-size: 12px;
}

.status-card strong {
  color: #ffffff;
}

.error-message {
  margin: 16px 0 0;
  color: #ff8d8d;
}

.status-bar {
  display: flex;
  gap: 20px;
  padding: 10px 24px;
  border-top: 1px solid rgba(255, 255, 255, 0.08);
  color: #7f918a;
  background: #0f1514;
  font-size: 12px;
}
</style>
