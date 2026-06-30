<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { open } from "@tauri-apps/plugin-dialog";
import EngineStatusPanel from "../components/EngineStatusPanel.vue";
import { getAria2ProcessStatus, pingAria2Rpc } from "../services/aria2";
import { createDownloadTask, listDownloadTasks } from "../services/tasks";
import type { AppInfo, BackendPing } from "../types/app";
import type { Aria2ProcessStatus, Aria2RpcStatus } from "../types/aria2";
import type { DownloadTask } from "../types/tasks";

const props = defineProps<{
  appInfo: AppInfo | null;
  backendPing: BackendPing | null;
  errorMessage: string;
}>();

const aria2Process = ref<Aria2ProcessStatus | null>(null);
const aria2Rpc = ref<Aria2RpcStatus | null>(null);
const tasks = ref<DownloadTask[]>([]);
const taskLoading = ref(false);
const showCreateDialog = ref(false);
const showDiagnostics = ref(false);
const newTaskUrl = ref("");
const newTaskFileName = ref("");
const newTaskSaveDir = ref("");
const startMode = ref<"now" | "paused">("now");
const note = ref("");
const formErrorMessage = ref("");
const toasts = ref<ToastMessage[]>([]);
const tableScroll = ref<HTMLElement | null>(null);
const activeInputType = ref("URL 下载");
const showAdvanced = ref(false);
let taskRefreshTimer: number | undefined;
let nextToastId = 1;
let lastRefreshErrorAt = 0;
let isDraggingTable = false;
let tableDragStartX = 0;
let tableDragStartScrollLeft = 0;
const notifiedErrorTaskKeys = new Set<string>();

type ToastType = "success" | "error" | "info";

interface ToastMessage {
  id: number;
  type: ToastType;
  message: string;
}

const inputTypes = ["URL 下载", "批量 URL", "种子文件（后期）", "磁力链接（后期）"];

const isUrlValid = computed(() => /^https?:\/\/.+/i.test(newTaskUrl.value.trim()));

async function refreshPhaseStatus() {
  const [process, rpc] = await Promise.all([getAria2ProcessStatus(), pingAria2Rpc()]);
  aria2Process.value = process;
  aria2Rpc.value = rpc;
}

async function refreshTasks(showError = false) {
  try {
    const nextTasks = await listDownloadTasks();
    notifyNewTaskErrors(tasks.value, nextTasks);
    tasks.value = nextTasks;
  } catch (error) {
    const now = Date.now();
    if (showError || now - lastRefreshErrorAt > 10000) {
      notify("error", getErrorMessage(error));
      lastRefreshErrorAt = now;
    }
  }
}

function notifyNewTaskErrors(previousTasks: DownloadTask[], nextTasks: DownloadTask[]) {
  const previousStatus = new Map(previousTasks.map((task) => [taskKey(task), task.status]));

  for (const task of nextTasks) {
    const key = taskKey(task);
    if (
      task.status === "error" &&
      previousStatus.get(key) !== "error" &&
      !notifiedErrorTaskKeys.has(key)
    ) {
      notifiedErrorTaskKeys.add(key);
      notify("error", `任务下载失败：${formatTaskError(task)}`);
    }
  }
}

function taskKey(task: DownloadTask) {
  return task.gid || String(task.id);
}

function openCreateDialog() {
  formErrorMessage.value = "";
  showCreateDialog.value = true;
}

function closeCreateDialog() {
  if (taskLoading.value) {
    return;
  }

  showCreateDialog.value = false;
}

async function selectSaveDir() {
  const selected = await open({
    directory: true,
    multiple: false,
    title: "选择下载目录",
  });

  if (typeof selected === "string") {
    newTaskSaveDir.value = selected;
  }
}

async function submitCreateTask() {
  if (!isUrlValid.value) {
    formErrorMessage.value = "请输入有效的 HTTP / HTTPS 下载链接";
    return;
  }

  taskLoading.value = true;
  formErrorMessage.value = "";

  try {
    const task = await createDownloadTask({
      url: newTaskUrl.value,
      fileName: newTaskFileName.value || null,
      saveDir: newTaskSaveDir.value || null,
    });
    tasks.value = [task, ...tasks.value.filter((item) => item.id !== task.id)];
    notify("success", "任务已添加");
    void refreshPhaseStatus();
    void refreshTasks();
    resetCreateForm();
    showCreateDialog.value = false;
  } catch (error) {
    notify("error", getErrorMessage(error));
  } finally {
    taskLoading.value = false;
  }
}

function notify(type: ToastType, message: string) {
  const toast: ToastMessage = {
    id: nextToastId++,
    type,
    message,
  };
  toasts.value = [...toasts.value, toast];

  window.setTimeout(
    () => {
      dismissToast(toast.id);
    },
    type === "error" ? 6200 : 3200,
  );
}

function dismissToast(id: number) {
  toasts.value = toasts.value.filter((toast) => toast.id !== id);
}

function getErrorMessage(error: unknown) {
  if (error instanceof Error) {
    return error.message;
  }

  const message = String(error);
  return message || "操作失败，请稍后重试";
}

function resetCreateForm() {
  newTaskUrl.value = "";
  newTaskFileName.value = "";
  newTaskSaveDir.value = "";
  startMode.value = "now";
  note.value = "";
  showAdvanced.value = false;
  formErrorMessage.value = "";
}

function formatTaskError(task: DownloadTask) {
  const code = task.errorCode ? `错误码 ${task.errorCode}：` : "";
  return `${code}${task.errorMessage || "未知错误"}`;
}

function formatSizePair(task: DownloadTask) {
  if (task.totalLength <= 0) {
    return `${formatSize(task.completedLength)} / 未知`;
  }

  return `${formatSize(task.completedLength)} / ${formatSize(task.totalLength)}`;
}

function startTableDrag(event: PointerEvent) {
  if (!tableScroll.value || event.button !== 0) {
    return;
  }

  isDraggingTable = true;
  tableDragStartX = event.clientX;
  tableDragStartScrollLeft = tableScroll.value.scrollLeft;
  tableScroll.value.setPointerCapture(event.pointerId);
}

function moveTableDrag(event: PointerEvent) {
  if (!isDraggingTable || !tableScroll.value) {
    return;
  }

  tableScroll.value.scrollLeft = tableDragStartScrollLeft - (event.clientX - tableDragStartX);
}

function stopTableDrag(event: PointerEvent) {
  if (!isDraggingTable) {
    return;
  }

  isDraggingTable = false;
  tableScroll.value?.releasePointerCapture(event.pointerId);
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
    return "0 B";
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

watch(
  () => props.errorMessage,
  (message) => {
    if (message) {
      notify("error", message);
    }
  },
);

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

        <section v-else class="task-table-shell">
          <div
            ref="tableScroll"
            class="task-table-scroll"
            @pointerdown="startTableDrag"
            @pointermove="moveTableDrag"
            @pointerup="stopTableDrag"
            @pointerleave="stopTableDrag"
            @pointercancel="stopTableDrag"
          >
            <div class="task-table-inner">
              <div class="task-table-head task-table-grid">
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
                <article v-for="task in tasks" :key="task.id" class="task-row task-table-grid">
                  <div class="task-title">
                    <strong>{{ task.fileName }}</strong>
                    <small>{{ task.url }}</small>
                    <small v-if="task.status === 'error'" class="task-error-detail">{{ formatTaskError(task) }}</small>
                  </div>
                  <span :class="['status-badge', `status-${task.status}`]">{{ formatStatus(task.status) }}</span>
                  <div class="progress-cell">
                    <div class="progress-bar"><span :style="{ width: `${taskProgress(task)}%` }" /></div>
                    <small>{{ taskProgress(task) }}%</small>
                  </div>
                  <span>{{ formatSizePair(task) }}</span>
                  <span>{{ formatSize(task.downloadSpeed) }}/s</span>
                  <span>{{ formatEta(task) }}</span>
                  <span class="task-path" :title="task.filePath || task.saveDir">{{ task.saveDir }}</span>
                  <button type="button" class="row-action" disabled>更多</button>
                </article>
              </div>
            </div>
          </div>
        </section>
      </main>
    </section>

    <button type="button" class="floating-add" aria-label="添加任务" @click="openCreateDialog">＋</button>

    <div class="toast-stack" aria-live="polite" aria-atomic="true">
      <div v-for="toast in toasts" :key="toast.id" :class="['toast', `toast-${toast.type}`]">
        <span class="toast-dot" aria-hidden="true" />
        <p>{{ toast.message }}</p>
        <button type="button" aria-label="关闭通知" @click="dismissToast(toast.id)">×</button>
      </div>
    </div>

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
          <div class="path-input-row">
            <input v-model="newTaskSaveDir" type="text" placeholder="留空使用 ~/Downloads，也可输入或选择目录" />
            <button type="button" class="secondary path-select-button" :disabled="taskLoading" @click="selectSaveDir">选择目录</button>
          </div>
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

        <p v-if="formErrorMessage" class="dialog-error">{{ formErrorMessage }}</p>

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

.task-table-shell {
  height: 100%;
  min-height: 0;
  overflow: hidden;
  background: #151515;
}

.task-table-scroll {
  height: 100%;
  min-height: 0;
  overflow-x: auto;
  overflow-y: hidden;
  cursor: grab;
}

.task-table-scroll:active {
  cursor: grabbing;
}

.task-table-inner {
  min-width: 1420px;
  height: 100%;
  min-height: 0;
  display: grid;
  grid-template-rows: auto minmax(0, 1fr);
}

.task-table-grid {
  display: grid;
  grid-template-columns: minmax(320px, 1.4fr) 90px 160px 160px 120px 110px minmax(260px, 0.8fr) 70px;
  gap: 18px;
  align-items: center;
}

.task-table-head {
  padding: 14px 18px;
  color: #899389;
  background: #1e2420;
  font-size: 12px;
  font-weight: 800;
}

.task-list-scroll {
  min-height: 0;
  overflow-y: auto;
  overflow-x: hidden;
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

.task-error-detail {
  margin-top: 4px;
  color: #ff8d8d !important;
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

.toast-stack {
  position: fixed;
  right: 24px;
  top: 70px;
  z-index: 30;
  width: min(360px, calc(100vw - 48px));
  display: grid;
  gap: 10px;
  pointer-events: none;
}

.toast {
  min-height: 48px;
  display: grid;
  grid-template-columns: auto minmax(0, 1fr) auto;
  align-items: center;
  gap: 10px;
  border: 1px solid rgba(255, 255, 255, 0.12);
  border-left-color: #66e39a;
  border-radius: 12px;
  padding: 12px 12px 12px 14px;
  color: #e7f1ec;
  background: rgba(21, 29, 26, 0.96);
  box-shadow: 0 16px 42px rgba(0, 0, 0, 0.42);
  pointer-events: auto;
}

.toast-error {
  border-left-color: #ff8d8d;
}

.toast-info {
  border-left-color: #8db8ff;
}

.toast-dot {
  width: 9px;
  height: 9px;
  border-radius: 999px;
  background: #66e39a;
}

.toast-error .toast-dot {
  background: #ff8d8d;
}

.toast-info .toast-dot {
  background: #8db8ff;
}

.toast p {
  margin: 0;
  overflow-wrap: anywhere;
  font-size: 14px;
  line-height: 1.4;
}

.toast button {
  width: 24px;
  height: 24px;
  border: 0;
  border-radius: 999px;
  color: #9dafaa;
  background: transparent;
  font-size: 18px;
  line-height: 1;
  cursor: pointer;
}

.toast button:hover {
  color: #e7f1ec;
  background: rgba(255, 255, 255, 0.08);
}

.dialog-error {
  margin: 0;
  color: #ff8d8d;
  font-size: 13px;
  line-height: 1.4;
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
  border-radius: 999px;
  padding: 10px 15px;
  color: #e7f1ec;
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
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 12px;
  padding: 12px 13px;
  color: #e7f1ec;
  background: rgba(255, 255, 255, 0.055);
}

.path-input-row {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  gap: 10px;
}

.path-select-button {
  min-width: 96px;
  white-space: nowrap;
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
