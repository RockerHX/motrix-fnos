<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
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
const showDiagnostics = ref(false);
const newTaskUrl = ref("");
const newTaskFileName = ref("");
const newTaskSaveDir = ref("");
const startMode = ref<"now" | "paused">("now");
const note = ref("");
const activeInputType = ref("URL 下载");
const showAdvanced = ref(false);
let taskRefreshTimer: number | undefined;

const inputTypes = ["URL 下载", "批量 URL", "种子文件（后期）", "磁力链接（后期）"];

const categories = computed(() => [
  { name: "全部", count: tasks.value.length },
  { name: "下载中", count: tasks.value.filter((task) => task.status === "active").length },
  { name: "已完成", count: tasks.value.filter((task) => task.status === "complete").length },
  { name: "做种", count: 0 },
  { name: "活动", count: tasks.value.filter((task) => task.status === "active").length },
  { name: "暂停", count: tasks.value.filter((task) => task.status === "paused").length },
  { name: "错误", count: tasks.value.filter((task) => task.status === "error").length },
]);

const isUrlValid = computed(() => /^https?:\/\/.+/i.test(newTaskUrl.value.trim()));

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
  if (!isUrlValid.value) {
    taskErrorMessage.value = "请输入有效的 HTTP / HTTPS 下载链接";
    return;
  }

  taskLoading.value = true;
  taskErrorMessage.value = "";

  try {
    const task = await createDownloadTask({
      url: newTaskUrl.value,
      fileName: newTaskFileName.value || null,
      saveDir: newTaskSaveDir.value || null,
    });
    tasks.value = [task, ...tasks.value.filter((item) => item.id !== task.id)];
    void refreshTasks();
    newTaskUrl.value = "";
    newTaskFileName.value = "";
    newTaskSaveDir.value = "";
    startMode.value = "now";
    note.value = "";
    showAdvanced.value = false;
    showCreateDialog.value = false;
  } catch (error) {
    taskErrorMessage.value = error instanceof Error ? error.message : String(error);
  } finally {
    taskLoading.value = false;
  }
}

function formatStatus(status: DownloadTask["status"]) {
  const labels: Record<DownloadTask["status"], string> = {
    pending: "排队",
    active: "下载中",
    paused: "暂停",
    complete: "已完成",
    error: "错误",
    removed: "已删除",
  };

  return labels[status];
}

function taskProgress(task: DownloadTask) {
  if (task.totalLength <= 0) {
    return 0;
  }

  return Math.min(100, Math.round((task.completedLength / task.totalLength) * 100));
}

function formatEta(task: DownloadTask) {
  if (task.downloadSpeed <= 0 || task.totalLength <= task.completedLength) {
    return "--";
  }

  const seconds = Math.ceil((task.totalLength - task.completedLength) / task.downloadSpeed);
  if (seconds < 60) {
    return `${seconds}s`;
  }

  const minutes = Math.floor(seconds / 60);
  const restSeconds = seconds % 60;
  return `${minutes}m ${restSeconds}s`;
}

function formatSize(size: number) {
  if (size <= 0) {
    return "0 B / 未知";
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
  taskRefreshTimer = window.setInterval(() => {
    void refreshTasks();
  }, 2000);
});

onBeforeUnmount(() => {
  if (taskRefreshTimer) {
    window.clearInterval(taskRefreshTimer);
  }
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
          <button type="button" class="ghost diagnostics-button" @click="showDiagnostics = true">诊断</button>
        </div>
      </header>

      <main class="task-content">
        <section class="task-table-shell">
          <div class="task-table-head">
            <span>任务名称</span>
            <span>状态</span>
            <span>进度</span>
            <span>已下载 / 总大小</span>
            <span>速度</span>
            <span>剩余时间</span>
            <span>保存路径</span>
            <span>操作</span>
          </div>

          <div class="task-list-scroll">
            <div v-if="tasks.length === 0" class="empty-state">
              <div class="empty-illustration">↓</div>
              <h2>暂无任务</h2>
              <p>添加 HTTP / HTTPS 链接开始下载，也可以直接粘贴链接创建任务。</p>
              <button type="button" class="primary" @click="openCreateDialog">添加任务</button>
            </div>

            <article v-for="task in tasks" v-else :key="task.id" class="task-row">
              <div class="task-title">
                <strong>{{ task.fileName }}</strong>
                <small>{{ task.url }}</small>
              </div>
              <span :class="['status-badge', `status-${task.status}`]">{{ formatStatus(task.status) }}</span>
              <div class="progress-cell">
                <div class="progress-bar"><span :style="{ width: `${taskProgress(task)}%` }" /></div>
                <small>{{ taskProgress(task) }}%</small>
              </div>
              <span>{{ formatSize(task.completedLength) }} / {{ formatSize(task.totalLength) }}</span>
              <span>{{ formatSize(task.downloadSpeed) }}/s</span>
              <span>{{ formatEta(task) }}</span>
              <span class="task-path">{{ task.saveDir ?? "默认目录待设置" }}</span>
              <button type="button" class="row-action" disabled>更多</button>
            </article>
          </div>
        </section>

        <p v-if="errorMessage" class="error-message">{{ errorMessage }}</p>
        <p v-if="taskErrorMessage" class="error-message">{{ taskErrorMessage }}</p>
      </main>

      <footer class="status-bar">
        <span>下载 0 B/s</span>
        <span>上传 0 B/s</span>
        <span>连接数 0</span>
        <span>任务数 {{ tasks.length }}</span>
        <span>磁盘状态：待接入</span>
      </footer>
    </section>

    <div v-if="showCreateDialog" class="dialog-backdrop" @click.self="closeCreateDialog">
      <form class="create-dialog" @submit.prevent="submitCreateTask">
        <div class="dialog-header">
          <div>
            <p class="eyebrow">New Task</p>
            <h2>新建下载任务</h2>
          </div>
          <button type="button" class="icon-button" :disabled="taskLoading" @click="closeCreateDialog">×</button>
        </div>

        <div class="input-tabs" aria-label="输入类型">
          <button
            v-for="type in inputTypes"
            :key="type"
            type="button"
            :class="{ active: activeInputType === type }"
            :disabled="type !== 'URL 下载'"
            @click="activeInputType = type"
          >
            {{ type }}
          </button>
        </div>

        <label>
          <span>下载链接</span>
          <input v-model="newTaskUrl" type="url" placeholder="https://example.com/file.zip" required />
          <small v-if="newTaskUrl && !isUrlValid" class="field-error">当前仅支持 HTTP / HTTPS 链接</small>
        </label>
        <label>
          <span>文件名</span>
          <input v-model="newTaskFileName" type="text" placeholder="留空则从链接自动识别" />
        </label>
        <label>
          <span>保存路径</span>
          <input v-model="newTaskSaveDir" type="text" placeholder="例如 /vol1/Downloads" />
        </label>

        <div class="segmented-field">
          <span>开始方式</span>
          <div class="segmented-control">
            <button type="button" :class="{ active: startMode === 'now' }" @click="startMode = 'now'">立即开始</button>
            <button type="button" :class="{ active: startMode === 'paused' }" @click="startMode = 'paused'">添加后暂停</button>
          </div>
        </div>

        <label>
          <span>备注</span>
          <input v-model="note" type="text" placeholder="可选" />
        </label>

        <button type="button" class="advanced-toggle" @click="showAdvanced = !showAdvanced">
          {{ showAdvanced ? "收起高级设置" : "展开高级设置" }}
        </button>
        <div v-if="showAdvanced" class="advanced-grid">
          <label><span>分类</span><input type="text" placeholder="默认" disabled /></label>
          <label><span>连接数</span><input type="number" placeholder="16" disabled /></label>
          <label><span>限速</span><input type="text" placeholder="不限速" disabled /></label>
          <label><span>代理</span><input type="text" placeholder="后期支持" disabled /></label>
        </div>

        <p v-if="taskErrorMessage" class="error-message">{{ taskErrorMessage }}</p>

        <div class="dialog-actions">
          <button type="button" :disabled="taskLoading" @click="closeCreateDialog">取消</button>
          <button type="submit" class="primary" :disabled="taskLoading || !isUrlValid">{{ taskLoading ? "创建中" : "开始下载" }}</button>
        </div>
      </form>
    </div>

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
  height: 100vh;
  overflow: hidden;
  display: grid;
  grid-template-columns: 240px minmax(0, 1fr);
  color: #e7f1ec;
  background: #0b1110;
}

.sidebar {
  padding: 24px 16px;
  border-right: 1px solid rgba(255, 255, 255, 0.08);
  background: #0d1513;
}

.brand {
  display: flex;
  align-items: center;
  gap: 13px;
  margin-bottom: 34px;
}

.brand-mark {
  width: 42px;
  height: 42px;
  display: grid;
  place-items: center;
  border-radius: 13px;
  color: #092216;
  background: #66e39a;
  font-size: 22px;
  font-weight: 900;
}

.brand strong,
.brand span {
  display: block;
}

.brand strong {
  font-size: 18px;
}

.brand span {
  margin-top: 4px;
  color: #91a19a;
  font-size: 13px;
}

.category-list {
  display: grid;
  gap: 10px;
}

.category-list button {
  width: 100%;
  display: flex;
  align-items: center;
  justify-content: space-between;
  border: 0;
  border-radius: 14px;
  padding: 13px 14px;
  color: #b2c1bb;
  background: transparent;
  text-align: left;
  cursor: pointer;
}

.category-list button.active,
.category-list button:hover {
  color: #ffffff;
  background: #173628;
}

.category-list em {
  min-width: 26px;
  border-radius: 999px;
  color: #a5bbb1;
  background: rgba(255, 255, 255, 0.08);
  font-style: normal;
  font-size: 12px;
  text-align: center;
}

.main-area {
  min-width: 0;
  min-height: 0;
  display: grid;
  grid-template-rows: auto minmax(0, 1fr) auto;
}

.toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 22px;
  padding: 24px 26px;
  border-bottom: 1px solid rgba(255, 255, 255, 0.08);
  background: #0d1513;
}

.eyebrow {
  margin: 0 0 6px;
  color: #66e39a;
  font-size: 12px;
  font-weight: 800;
  letter-spacing: 0.1em;
  text-transform: uppercase;
}

h1,
h2 {
  margin: 0;
  color: #ffffff;
}

h1 {
  font-size: 28px;
}

.toolbar-actions {
  display: flex;
  align-items: center;
  gap: 9px;
}

button,
input {
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 999px;
  padding: 10px 15px;
  color: #e7f1ec;
  background: rgba(255, 255, 255, 0.055);
  font: inherit;
}

button {
  cursor: pointer;
}

button:disabled,
input:disabled {
  cursor: not-allowed;
  opacity: 0.46;
}

.primary {
  border-color: transparent;
  color: #092216;
  background: #66e39a;
  font-weight: 900;
}

.ghost {
  color: #bcd0c7;
  background: transparent;
}

.diagnostics-button {
  padding-inline: 12px;
}

.task-content {
  min-height: 0;
  overflow: hidden;
  padding: 26px;
}

.task-table-shell {
  height: 100%;
  min-height: 0;
  overflow: hidden;
  display: grid;
  grid-template-rows: auto minmax(0, 1fr);
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 18px;
  background: #111b18;
}

.task-table-head,
.task-row {
  display: grid;
  grid-template-columns: minmax(220px, 1.4fr) 84px minmax(130px, 0.8fr) 130px 90px 90px minmax(150px, 0.8fr) 70px;
  gap: 14px;
  align-items: center;
}

.task-table-head {
  padding: 14px 18px;
  color: #7f918a;
  background: rgba(255, 255, 255, 0.04);
  font-size: 12px;
  font-weight: 800;
}

.task-list-scroll {
  min-height: 0;
  overflow: auto;
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
.task-path,
.progress-cell small {
  color: #84968f;
}

.status-badge {
  width: fit-content;
  border-radius: 999px;
  padding: 5px 9px;
  color: #2c2410;
  background: #dfb84a;
  font-size: 12px;
  font-weight: 900;
}

.status-active,
.status-complete {
  color: #092216;
  background: #66e39a;
}

.status-paused,
.status-removed {
  color: #dbe7e1;
  background: #4d5d58;
}

.status-error {
  color: #2a0909;
  background: #ff8d8d;
}

.progress-cell {
  display: grid;
  gap: 6px;
}

.progress-bar {
  overflow: hidden;
  height: 7px;
  border-radius: 999px;
  background: rgba(255, 255, 255, 0.08);
}

.progress-bar span {
  display: block;
  width: 0%;
  height: 100%;
  background: #66e39a;
}

.row-action {
  padding: 7px 9px;
  font-size: 12px;
}

.empty-state {
  min-height: 100%;
  display: grid;
  justify-items: center;
  align-content: center;
  padding: 36px;
  text-align: center;
}

.empty-illustration {
  width: 82px;
  height: 82px;
  display: grid;
  place-items: center;
  margin-bottom: 22px;
  border-radius: 26px;
  color: #66e39a;
  background: #173628;
  font-size: 56px;
  line-height: 1;
}

.empty-state p {
  max-width: 520px;
  margin: 14px 0 24px;
  color: #8fa29a;
  font-size: 17px;
  line-height: 1.55;
}

.error-message {
  margin: 14px 0 0;
  color: #ff8d8d;
}

.status-bar {
  display: flex;
  gap: 22px;
  padding: 10px 24px;
  border-top: 1px solid rgba(255, 255, 255, 0.08);
  color: #7f918a;
  background: #0d1513;
  font-size: 12px;
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

.create-dialog,
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

.input-tabs,
.segmented-control {
  display: flex;
  gap: 8px;
  padding: 4px;
  border-radius: 999px;
  background: rgba(255, 255, 255, 0.05);
}

.input-tabs button,
.segmented-control button {
  border: 0;
  background: transparent;
}

.input-tabs button.active,
.segmented-control button.active {
  color: #092216;
  background: #66e39a;
  font-weight: 900;
}

.create-dialog label,
.segmented-field {
  display: grid;
  gap: 8px;
  color: #9dafaa;
  font-size: 13px;
  font-weight: 800;
}

.create-dialog input {
  width: 100%;
  border-radius: 12px;
  padding: 12px 13px;
}

.field-error {
  color: #ff8d8d;
}

.advanced-toggle {
  width: fit-content;
  border: 0;
  padding: 0;
  color: #66e39a;
  background: transparent;
  font-weight: 800;
}

.advanced-grid,
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

.dialog-actions {
  justify-content: flex-end;
}
</style>
