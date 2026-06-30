<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import EngineStatusPanel from "../components/EngineStatusPanel.vue";
import { getAria2ProcessStatus, pingAria2Rpc } from "../services/aria2";
import { createDownloadTask, listDownloadTasks } from "../services/tasks";
import type { AppInfo, BackendPing } from "../types/app";
import type { Aria2ProcessStatus, Aria2RpcStatus } from "../types/aria2";
import type { DownloadTask } from "../types/tasks";

defineProps<{
  appInfo: AppInfo | null;
  backendPing: BackendPing | null;
  errorMessage: string;
}>();

const aria2Process = ref<Aria2ProcessStatus | null>(null);
const aria2Rpc = ref<Aria2RpcStatus | null>(null);
const tasks = ref<DownloadTask[]>([]);
const taskErrorMessage = ref("");
const taskLoading = ref(false);
const showCreateDialog = ref(false);
const newTaskUrl = ref("");
const newTaskFileName = ref("");
const newTaskSaveDir = ref("");

const categories = computed(() => [
  { name: "全部", count: tasks.value.length },
  { name: "下载中", count: 0 },
  { name: "已完成", count: 0 },
  { name: "做种", count: 0 },
  { name: "活动", count: 0 },
  { name: "暂停", count: 0 },
  { name: "错误", count: 0 },
]);

async function refreshPhaseStatus() {
  const [process, rpc] = await Promise.all([getAria2ProcessStatus(), pingAria2Rpc()]);
  aria2Process.value = process;
  aria2Rpc.value = rpc;
}

async function refreshTasks() {
  taskErrorMessage.value = "";
  tasks.value = await listDownloadTasks();
}

function openCreateDialog() {
  taskErrorMessage.value = "";
  showCreateDialog.value = true;
}

function closeCreateDialog() {
  if (taskLoading.value) {
    return;
  }

  showCreateDialog.value = false;
}

async function submitCreateTask() {
  taskLoading.value = true;
  taskErrorMessage.value = "";

  try {
    const task = await createDownloadTask({
      url: newTaskUrl.value,
      fileName: newTaskFileName.value || null,
      saveDir: newTaskSaveDir.value || null,
    });
    tasks.value = [task, ...tasks.value.filter((item) => item.id !== task.id)];
    newTaskUrl.value = "";
    newTaskFileName.value = "";
    newTaskSaveDir.value = "";
    showCreateDialog.value = false;
  } catch (error) {
    taskErrorMessage.value = error instanceof Error ? error.message : String(error);
  } finally {
    taskLoading.value = false;
  }
}

function formatStatus(status: DownloadTask["status"]) {
  if (status === "pending") {
    return "待开始";
  }

  return status;
}

function formatSize(size: number) {
  if (size <= 0) {
    return "-";
  }

  const units = ["B", "KB", "MB", "GB", "TB"];
  let value = size;
  let unitIndex = 0;

  while (value >= 1024 && unitIndex < units.length - 1) {
    value /= 1024;
    unitIndex += 1;
  }

  return `${value.toFixed(value >= 10 || unitIndex === 0 ? 0 : 1)} ${units[unitIndex]}`;
}

onMounted(() => {
  void refreshPhaseStatus();
  void refreshTasks();
});
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
          <p class="eyebrow">阶段 1 / MVP 下载闭环</p>
          <h1>全部任务</h1>
        </div>
        <div class="toolbar-actions">
          <button type="button" class="primary" @click="openCreateDialog">添加任务</button>
          <button type="button" disabled>开始</button>
          <button type="button" disabled>暂停</button>
          <button type="button" disabled>删除</button>
          <input type="search" placeholder="搜索任务" disabled />
        </div>
      </header>

      <main class="content-stack">
        <section v-if="tasks.length === 0" class="empty-state">
          <div class="empty-icon">↓</div>
          <h2>暂无任务</h2>
          <p>阶段 1 已支持创建 HTTP / HTTPS 下载任务记录，后续会接入 Aria2 Next 执行真实下载。</p>
          <button type="button" class="primary" @click="openCreateDialog">添加第一个任务</button>
        </section>

        <section v-else class="task-list-panel">
          <div class="task-list-header">
            <span>任务</span>
            <span>状态</span>
            <span>大小</span>
            <span>保存位置</span>
          </div>
          <article v-for="task in tasks" :key="task.id" class="task-row">
            <div class="task-title">
              <strong>{{ task.fileName }}</strong>
              <small>{{ task.url }}</small>
            </div>
            <span class="task-status">{{ formatStatus(task.status) }}</span>
            <span>{{ formatSize(task.totalLength) }}</span>
            <span class="task-path">{{ task.saveDir ?? "默认目录待设置" }}</span>
          </article>
        </section>

        <section class="phase-status">
          <div class="phase-header">
            <div>
              <p class="eyebrow">Status Check</p>
              <h2>阶段状态检查</h2>
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
            <div class="status-card">
              <span>任务数量</span>
              <strong>{{ tasks.length }}</strong>
            </div>
            <div class="status-card">
              <span>Aria2 进程</span>
              <strong>{{ aria2Process?.running ? "运行中" : "未运行" }}</strong>
            </div>
            <div class="status-card">
              <span>Aria2 RPC</span>
              <strong>{{ aria2Rpc?.connected ? "已连接" : "未连接" }}</strong>
            </div>
          </div>

          <p v-if="errorMessage" class="error-message">{{ errorMessage }}</p>
          <p v-if="taskErrorMessage" class="error-message">{{ taskErrorMessage }}</p>
        </section>

        <EngineStatusPanel />
      </main>

      <footer class="status-bar">
        <span>下载 0 B/s</span>
        <span>上传 0 B/s</span>
        <span>任务数 {{ tasks.length }}</span>
        <span>磁盘状态：待接入</span>
      </footer>
    </section>

    <div v-if="showCreateDialog" class="dialog-backdrop" @click.self="closeCreateDialog">
      <form class="create-dialog" @submit.prevent="submitCreateTask">
        <div class="dialog-header">
          <div>
            <p class="eyebrow">New Task</p>
            <h2>添加下载任务</h2>
          </div>
          <button type="button" class="icon-button" :disabled="taskLoading" @click="closeCreateDialog">×</button>
        </div>

        <label>
          <span>下载链接</span>
          <input v-model="newTaskUrl" type="url" placeholder="https://example.com/file.zip" required />
        </label>
        <label>
          <span>文件名（可选）</span>
          <input v-model="newTaskFileName" type="text" placeholder="留空则从链接推断" />
        </label>
        <label>
          <span>保存路径（可选）</span>
          <input v-model="newTaskSaveDir" type="text" placeholder="例如 /vol1/Downloads" />
        </label>

        <p v-if="taskErrorMessage" class="error-message">{{ taskErrorMessage }}</p>

        <div class="dialog-actions">
          <button type="button" :disabled="taskLoading" @click="closeCreateDialog">取消</button>
          <button type="submit" class="primary" :disabled="taskLoading">{{ taskLoading ? "创建中" : "创建任务" }}</button>
        </div>
      </form>
    </div>
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
  width: 100%;
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
.phase-status,
.task-list-panel {
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

.task-list-panel {
  overflow: hidden;
}

.task-list-header,
.task-row {
  display: grid;
  grid-template-columns: minmax(260px, 1fr) 100px 100px minmax(160px, 0.6fr);
  gap: 14px;
  align-items: center;
}

.task-list-header {
  padding: 14px 18px;
  color: #7f918a;
  background: rgba(255, 255, 255, 0.04);
  font-size: 12px;
  font-weight: 700;
}

.task-row {
  padding: 16px 18px;
  border-top: 1px solid rgba(255, 255, 255, 0.06);
}

.task-title {
  min-width: 0;
}

.task-title strong,
.task-title small {
  overflow: hidden;
  display: block;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.task-title small,
.task-path {
  color: #83958e;
}

.task-status {
  width: fit-content;
  border-radius: 999px;
  padding: 5px 9px;
  color: #cfeedd;
  background: rgba(103, 220, 160, 0.12);
  font-size: 12px;
  font-weight: 800;
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
  grid-template-columns: repeat(auto-fit, minmax(180px, 1fr));
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

.dialog-backdrop {
  position: fixed;
  inset: 0;
  display: grid;
  place-items: center;
  padding: 24px;
  background: rgba(0, 0, 0, 0.62);
}

.create-dialog {
  width: min(560px, 100%);
  display: grid;
  gap: 16px;
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 20px;
  padding: 22px;
  background: #151b1a;
  box-shadow: 0 24px 80px rgba(0, 0, 0, 0.45);
}

.dialog-header,
.dialog-actions {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.icon-button {
  width: 36px;
  height: 36px;
  padding: 0;
  font-size: 22px;
}

.create-dialog label {
  display: grid;
  gap: 8px;
  color: #9dafaa;
  font-size: 13px;
  font-weight: 700;
}

.create-dialog input {
  width: 100%;
  border-radius: 12px;
  padding: 12px 13px;
}

.dialog-actions {
  justify-content: flex-end;
}
</style>
