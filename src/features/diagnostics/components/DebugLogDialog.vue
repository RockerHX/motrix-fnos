<script setup lang="ts">
import { storeToRefs } from "pinia";
import { NButton, NCard, NEmpty, NModal, NTag, useMessage } from "naive-ui";
import { nextTick, ref, watch } from "vue";
import { useDebugLogStore } from "../stores/debugLogStore";
import type { DebugLogEntry, DebugLogLevel } from "../types";

const props = defineProps<{
  show: boolean;
}>();

const emit = defineEmits<{
  "update:show": [show: boolean];
}>();

const message = useMessage();
const debugLogStore = useDebugLogStore();
const { logs, isLoading, isClearing } = storeToRefs(debugLogStore);
const logListRef = ref<HTMLElement | null>(null);
const manualCopyRef = ref<HTMLTextAreaElement | null>(null);
const showManualCopy = ref(false);
const manualCopyText = ref("");

watch(
  () => props.show,
  (show) => {
    if (show) {
      void refreshLogs();
    }
  },
);

watch(
  () => logs.value.length,
  () => {
    if (props.show) {
      void scrollToBottom();
    }
  },
);

function updateShow(show: boolean) {
  emit("update:show", show);
}

function closeDialog() {
  updateShow(false);
}

async function refreshLogs() {
  try {
    await debugLogStore.refreshLogs();
    await scrollToBottom();
  } catch (error) {
    message.error(getErrorMessage(error));
  }
}

async function clearLogs() {
  try {
    await debugLogStore.clearLogs();
    message.success("调试日志已清空");
  } catch (error) {
    message.error(getErrorMessage(error));
  }
}

async function copyAllLogs() {
  if (logs.value.length === 0) {
    message.warning("当前没有可复制的调试日志");
    return;
  }

  const text = formatAllLogs();
  try {
    await copyText(text);
    message.success("调试日志已复制");
  } catch (error) {
    showManualCopyDialog(text);
    message.warning(`飞牛窗口限制自动复制：${getErrorMessage(error)}`);
  }
}

async function copyText(text: string) {
  if (navigator.clipboard?.writeText) {
    await navigator.clipboard.writeText(text);
    return;
  }

  throw new Error("当前飞牛窗口不提供剪贴板写入 API");
}

function showManualCopyDialog(text: string) {
  manualCopyText.value = text;
  showManualCopy.value = true;
  void nextTick(() => {
    manualCopyRef.value?.focus();
    manualCopyRef.value?.select();
  });
}

function closeManualCopyDialog() {
  showManualCopy.value = false;
}

function downloadAllLogs() {
  if (logs.value.length === 0) {
    message.warning("当前没有可下载的调试日志");
    return;
  }

  const blob = new Blob([formatAllLogs()], { type: "text/plain;charset=utf-8" });
  const url = URL.createObjectURL(blob);
  const link = document.createElement("a");
  link.href = url;
  link.download = `motrix-fnos-debug-${new Date().toISOString().replace(/[:.]/g, "-")}.log`;
  document.body.appendChild(link);
  link.click();
  document.body.removeChild(link);
  URL.revokeObjectURL(url);
  message.success("调试日志已导出");
}

function formatAllLogs() {
  return logs.value.map(formatLogLine).join("\n");
}

async function scrollToBottom() {
  await nextTick();
  const logList = logListRef.value;
  if (logList) {
    logList.scrollTop = logList.scrollHeight;
  }
}

function formatLogLine(log: DebugLogEntry) {
  return `[${formatTime(log.timestampMs)}] [${log.level.toUpperCase()}] [${log.module}] ${log.message}`;
}

function formatTime(timestampMs: number) {
  return new Date(timestampMs).toLocaleString();
}

function levelLabel(level: DebugLogLevel) {
  const labels: Record<DebugLogLevel, string> = {
    info: "INFO",
    warn: "WARN",
    error: "ERROR",
  };
  return labels[level];
}

function levelType(level: DebugLogLevel) {
  const types: Record<DebugLogLevel, "info" | "warning" | "error"> = {
    info: "info",
    warn: "warning",
    error: "error",
  };
  return types[level];
}

function getErrorMessage(error: unknown) {
  if (error instanceof Error) {
    return error.message;
  }

  const text = String(error);
  return text || "未知错误";
}
</script>

<template>
  <NModal :show="show" @update:show="updateShow">
    <NCard class="debug-log-dialog" role="dialog" aria-modal="true">
      <template #header>
        <div>
          <p class="eyebrow">Debug Logs</p>
          <h2>应用内调试日志</h2>
        </div>
      </template>
      <template #header-extra>
        <div class="header-actions">
          <NButton size="small" secondary :loading="isLoading" @click="refreshLogs">刷新</NButton>
          <NButton size="small" secondary @click="copyAllLogs">复制全部</NButton>
          <NButton size="small" secondary @click="downloadAllLogs">下载日志</NButton>
          <NButton size="small" secondary type="warning" :loading="isClearing" @click="clearLogs">清空</NButton>
          <NButton quaternary circle @click="closeDialog">×</NButton>
        </div>
      </template>

      <NEmpty v-if="logs.length === 0" description="暂无调试日志" />
      <div v-else ref="logListRef" class="log-list">
        <article v-for="log in logs" :key="log.id" class="log-entry" :class="`level-${log.level}`">
          <div class="log-meta">
            <span>{{ formatTime(log.timestampMs) }}</span>
            <NTag :type="levelType(log.level)" size="small" round>{{ levelLabel(log.level) }}</NTag>
            <code>{{ log.module }}</code>
          </div>
          <p>{{ log.message }}</p>
        </article>
      </div>
    </NCard>
  </NModal>

  <NModal :show="showManualCopy" @update:show="showManualCopy = $event">
    <NCard class="manual-copy-dialog" role="dialog" aria-modal="true">
      <template #header>
        <div>
          <p class="eyebrow">Manual Copy</p>
          <h2>手动复制调试日志</h2>
        </div>
      </template>
      <template #header-extra>
        <NButton quaternary circle @click="closeManualCopyDialog">×</NButton>
      </template>

      <p class="manual-copy-hint">飞牛内嵌窗口限制了自动写入剪贴板。下面文本已自动选中，请按 Command+C 或 Ctrl+C 手动复制；也可以使用“下载日志”。</p>
      <textarea ref="manualCopyRef" class="manual-copy-textarea" readonly :value="manualCopyText" />
      <div class="manual-copy-actions">
        <NButton secondary @click="downloadAllLogs">下载日志</NButton>
        <NButton type="primary" @click="closeManualCopyDialog">完成</NButton>
      </div>
    </NCard>
  </NModal>
</template>

<style scoped>
.debug-log-dialog {
  width: min(980px, calc(100vw - 48px));
  max-height: calc(100vh - 48px);
}

.manual-copy-dialog {
  width: min(900px, calc(100vw - 48px));
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

.log-list {
  max-height: min(620px, calc(100vh - 190px));
  overflow: auto;
  display: grid;
  gap: 10px;
  padding-right: 6px;
}

.log-entry {
  padding: 12px;
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-left: 3px solid #5d7280;
  border-radius: 12px;
  background: rgba(255, 255, 255, 0.04);
}

.log-entry.level-warn {
  border-left-color: #f2c97d;
}

.log-entry.level-error {
  border-left-color: #ff8d8d;
}

.log-meta {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 8px;
  color: #8d9c96;
  font-size: 12px;
}

.log-meta code {
  color: #9dd7ff;
}

.log-entry p {
  margin: 8px 0 0;
  color: #edf5ef;
  line-height: 1.6;
  white-space: pre-wrap;
  word-break: break-word;
}

.manual-copy-hint {
  margin: 0 0 12px;
  color: #b8c4be;
  line-height: 1.6;
}

.manual-copy-textarea {
  width: 100%;
  min-height: min(460px, calc(100vh - 260px));
  resize: vertical;
  border: 1px solid rgba(255, 255, 255, 0.12);
  border-radius: 12px;
  padding: 12px;
  color: #edf5ef;
  background: rgba(0, 0, 0, 0.28);
  font: 12px/1.6 ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, "Liberation Mono", monospace;
}

.manual-copy-actions {
  display: flex;
  justify-content: flex-end;
  gap: 10px;
  margin-top: 14px;
}
</style>
