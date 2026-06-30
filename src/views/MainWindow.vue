<script setup lang="ts">
import { storeToRefs } from "pinia";
import { onMounted, ref, watch } from "vue";
import { useMessage } from "naive-ui";
import EngineStatusPanel from "../components/EngineStatusPanel.vue";
import TaskCreateDialog from "../features/tasks/components/TaskCreateDialog.vue";
import TaskTable from "../features/tasks/components/TaskTable.vue";
import { useTaskPolling } from "../features/tasks/composables/useTaskPolling";
import { useTaskStore } from "../features/tasks/stores/taskStore";
import { getAria2ProcessStatus, pingAria2Rpc } from "../services/aria2";
import type { AppInfo, BackendPing } from "../types/app";
import type { Aria2ProcessStatus, Aria2RpcStatus } from "../types/aria2";

const props = defineProps<{
  appInfo: AppInfo | null;
  backendPing: BackendPing | null;
  errorMessage: string;
}>();

const message = useMessage();
const taskStore = useTaskStore();
const { tasks } = storeToRefs(taskStore);
const aria2Process = ref<Aria2ProcessStatus | null>(null);
const aria2Rpc = ref<Aria2RpcStatus | null>(null);
const showCreateDialog = ref(false);
const showDiagnostics = ref(false);

const { refresh: refreshTasks } = useTaskPolling({
  onRefreshError: (errorMessage) => message.error(errorMessage),
  onTaskError: (errorMessage) => message.error(errorMessage),
});

async function refreshPhaseStatus() {
  const [process, rpc] = await Promise.all([getAria2ProcessStatus(), pingAria2Rpc()]);
  aria2Process.value = process;
  aria2Rpc.value = rpc;
}

function openCreateDialog() {
  showCreateDialog.value = true;
}

async function handleTaskCreated() {
  message.success("任务已添加");
  void refreshPhaseStatus();
  await refreshTasks();
}


watch(
  () => props.errorMessage,
  (nextMessage) => {
    if (nextMessage) {
      message.error(nextMessage);
    }
  },
);

onMounted(() => {
  void refreshPhaseStatus();
});
</script>

<template>
  <div class="window-shell">
    <aside class="sidebar">
      <div class="brand">
        <div class="brand-mark" />
        <div>
          <strong>{{ appInfo?.name ?? "Motrix FNOS" }}</strong>
          <span>v{{ appInfo?.version ?? "2.1.0" }}</span>
        </div>
      </div>

      <nav class="category-list" aria-label="任务分类">
        <button type="button" class="active">
          <span class="nav-icon">⇩</span>
          <span>Downloading</span>
        </button>
        <button type="button">
          <span class="nav-icon">✓</span>
          <span>Completed</span>
        </button>
        <button type="button">
          <span class="nav-icon">Ⅱ</span>
          <span>Stopped</span>
        </button>
        <button type="button" class="nav-spaced">
          <span class="nav-icon">♜</span>
          <span>Trash</span>
        </button>
        <button type="button">
          <span class="nav-icon">♧</span>
          <span>Extensions</span>
        </button>
      </nav>

      <div class="sidebar-footer">
        <button type="button">
          <span class="nav-icon">⚙</span>
          <span>Settings</span>
        </button>
        <button type="button">
          <span class="nav-icon">?</span>
          <span>Help</span>
        </button>
      </div>
    </aside>

    <section class="main-area">
      <header class="topbar">
        <div class="topbar-spacer" />
        <div class="topbar-actions">
          <button type="button" title="筛选">≡</button>
          <button type="button" title="排序">≡</button>
          <button type="button" title="诊断" @click="showDiagnostics = true">⋮</button>
        </div>
      </header>

      <main class="content-stage">
        <section v-if="tasks.length === 0" class="empty-guide">
          <div class="empty-box" aria-hidden="true">
            <div class="box-lid" />
            <div class="box-body">
              <span>+</span>
            </div>
          </div>
          <h1>暂无任务</h1>
          <p>点击下方按钮或粘贴 HTTP / HTTPS 链接开始您的第一次下载。</p>
          <div class="empty-actions">
            <button type="button" class="primary" @click="openCreateDialog">
              <span>＋</span>
              添加任务
            </button>
            <button type="button" class="secondary">
              <span>⚙</span>
              打开设置
            </button>
          </div>
        </section>

        <TaskTable v-else :tasks="tasks" />
      </main>
    </section>

    <button type="button" class="floating-add" aria-label="添加任务" @click="openCreateDialog">＋</button>

    <TaskCreateDialog v-model:show="showCreateDialog" @created="handleTaskCreated" />

    <div v-if="showDiagnostics" class="dialog-backdrop" @click.self="showDiagnostics = false">
      <section class="diagnostics-dialog">
        <div class="dialog-header">
          <div>
            <p class="eyebrow">Diagnostics</p>
            <h2>阶段状态与引擎诊断</h2>
          </div>
          <button type="button" class="icon-button" @click="showDiagnostics = false">×</button>
        </div>
        <div class="diagnostics-grid">
          <div><span>应用版本</span><strong>{{ appInfo?.version ?? "-" }}</strong></div>
          <div><span>后端状态</span><strong>{{ appInfo?.backendStatus ?? "checking" }}</strong></div>
          <div><span>通信结果</span><strong>{{ backendPing?.message ?? "等待响应" }}</strong></div>
          <div><span>Aria2 进程</span><strong>{{ aria2Process?.running ? "运行中" : "未运行" }}</strong></div>
          <div><span>Aria2 RPC</span><strong>{{ aria2Rpc?.connected ? "已连接" : "未连接" }}</strong></div>
        </div>
        <EngineStatusPanel />
      </section>
    </div>
  </div>
</template>

<style scoped>
.window-shell {
  position: relative;
  height: 100vh;
  overflow: hidden;
  display: grid;
  grid-template-columns: 220px minmax(0, 1fr);
  color: #d7dfd8;
  background: #121212;
}

.sidebar {
  min-height: 0;
  display: grid;
  grid-template-rows: auto minmax(0, 1fr) auto;
  padding: 22px 8px 24px;
  border-right: 1px solid #324036;
  background: #0f100f;
}

.brand {
  display: flex;
  align-items: center;
  gap: 9px;
  padding: 0 22px 30px;
}

.brand-mark {
  width: 28px;
  height: 28px;
  border-radius: 4px;
  background: #6ab75f;
}

.brand strong,
.brand span {
  display: block;
}

.brand strong {
  color: #8ef08a;
  font-size: 22px;
  font-weight: 800;
  line-height: 1;
}

.brand span {
  margin-top: 4px;
  color: #d8e0d7;
  font-size: 12px;
}

.category-list,
.sidebar-footer {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.category-list button,
.sidebar-footer button {
  display: flex;
  align-items: center;
  gap: 14px;
  width: 100%;
  border: 0;
  border-radius: 6px;
  padding: 10px 12px;
  color: #cfd8ce;
  background: transparent;
  font: inherit;
  font-size: 16px;
  text-align: left;
  cursor: pointer;
}

.category-list button.active {
  color: #8ef08a;
  background: #4a4b48;
}

.nav-spaced {
  margin-top: 28px;
}

.nav-icon {
  width: 22px;
  color: currentColor;
  text-align: center;
  font-weight: 800;
}

.sidebar-footer {
  margin: 0 0 0;
  padding: 26px 8px 0;
  border-top: 1px solid #39443b;
}

.main-area {
  min-width: 0;
  min-height: 0;
  display: grid;
  grid-template-rows: 52px minmax(0, 1fr);
  background: #151515;
}

.topbar {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  border-bottom: 1px solid #324036;
  background: #151515;
}

.topbar-actions {
  display: flex;
  align-items: center;
  gap: 16px;
  padding-right: 26px;
}

.topbar-actions button {
  border: 0;
  padding: 4px;
  color: #cfd8ce;
  background: transparent;
  font-size: 23px;
  line-height: 1;
  cursor: pointer;
}

.content-stage {
  min-height: 0;
  overflow: hidden;
  position: relative;
}

.empty-guide {
  height: 100%;
  display: grid;
  justify-items: center;
  align-content: center;
  padding-bottom: 24px;
  text-align: center;
}

.empty-box {
  position: relative;
  width: 120px;
  height: 120px;
  margin-bottom: 44px;
  color: #565e55;
}

.box-lid {
  position: absolute;
  left: 20px;
  top: 6px;
  width: 78px;
  height: 32px;
  border: 4px solid #3d423d;
  border-bottom: 0;
  transform: skewX(-38deg);
}

.box-body {
  position: absolute;
  left: 10px;
  top: 34px;
  width: 100px;
  height: 88px;
  display: grid;
  place-items: center;
  border: 4px solid #3d423d;
  border-radius: 0 0 18px 18px;
}

.box-body span {
  width: 28px;
  height: 28px;
  display: grid;
  place-items: center;
  border-radius: 999px;
  color: #101710;
  background: #68ae5a;
  font-size: 26px;
  font-weight: 700;
  line-height: 1;
}

.empty-guide h1 {
  margin: 0 0 14px;
  color: #f1f2ed;
  font-size: 24px;
  font-weight: 400;
}

.empty-guide p {
  max-width: 360px;
  margin: 0 0 30px;
  color: #b7bfb4;
  font-size: 14px;
  line-height: 1.5;
}

.empty-actions {
  display: flex;
  justify-content: center;
  gap: 14px;
}

button,
input {
  font: inherit;
}

.primary,
.secondary {
  min-width: 118px;
  border-radius: 7px;
  padding: 10px 18px;
  font-size: 16px;
  cursor: pointer;
}

.primary {
  border: 1px solid #68ae5a;
  color: #101710;
  background: #68ae5a;
}

.secondary {
  border: 1px solid #3d423d;
  color: #dbe3d8;
  background: transparent;
}

.floating-add {
  position: absolute;
  right: 26px;
  bottom: 24px;
  width: 52px;
  height: 52px;
  border: 0;
  border-radius: 999px;
  color: #101710;
  background: #68ae5a;
  font-size: 30px;
  line-height: 1;
  box-shadow: 0 10px 30px rgba(0, 0, 0, 0.35);
  cursor: pointer;
}

.dialog-backdrop {
  position: fixed;
  inset: 0;
  z-index: 10;
  display: grid;
  place-items: center;
  padding: 24px;
  background: rgba(0, 0, 0, 0.66);
}

.diagnostics-dialog {
  width: min(640px, 100%);
  max-height: calc(100vh - 48px);
  overflow: auto;
  display: grid;
  gap: 16px;
  border: 1px solid rgba(255, 255, 255, 0.12);
  border-radius: 18px;
  padding: 22px;
  background: #151d1a;
  box-shadow: 0 24px 80px rgba(0, 0, 0, 0.48);
}

.diagnostics-dialog {
  width: min(900px, 100%);
}

.dialog-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.eyebrow {
  margin: 0 0 6px;
  color: #66e39a;
  font-size: 12px;
  font-weight: 800;
  letter-spacing: 0.1em;
  text-transform: uppercase;
}

.icon-button {
  width: 36px;
  height: 36px;
  padding: 0;
  font-size: 22px;
}

.diagnostics-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 12px;
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
