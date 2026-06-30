<script setup lang="ts">
import { computed, h, onMounted, ref } from "vue";
import { NDataTable } from "naive-ui";
import type { DataTableColumns } from "naive-ui";
import TaskActions from "./TaskActions.vue";
import TaskProgressCell from "./TaskProgressCell.vue";
import TaskStatusBadge from "./TaskStatusBadge.vue";
import { getUiPreferences, saveUiPreferences } from "../../../services/settings";
import type { DownloadTask } from "../../../types/tasks";

const props = defineProps<{
  tasks: DownloadTask[];
}>();

const defaultColumnWidths: Record<string, number> = {
  name: 360,
  status: 110,
  progress: 180,
  size: 180,
  speed: 130,
  eta: 120,
  actions: 260,
};

const columns = ref<DataTableColumns<DownloadTask>>(createColumns(defaultColumnWidths));
const scrollX = computed(() =>
  columns.value.reduce((total, column) => total + normalizeColumnWidth(column.width), 0),
);

onMounted(async () => {
  try {
    const preferences = await getUiPreferences();
    columns.value = createColumns({
      ...defaultColumnWidths,
      ...preferences.taskTableColumnWidths,
    });
  } catch {
    columns.value = createColumns(defaultColumnWidths);
  }
});

function createColumns(widths: Record<string, number>): DataTableColumns<DownloadTask> {
  return [
    {
    key: "name",
    title: "任务名称",
    width: widths.name,
    minWidth: 240,
    resizable: true,
    render: (task) =>
      h("div", { class: "task-name-cell" }, [
        h("strong", task.fileName),
        h("small", task.url),
        task.status === "error" ? h("small", { class: "task-error-detail" }, formatTaskError(task)) : null,
      ]),
    },
    {
    key: "status",
    title: "状态",
    width: widths.status,
    minWidth: 90,
    resizable: true,
    render: (task) => h(TaskStatusBadge, { status: task.status }),
    },
    {
    key: "progress",
    title: "进度",
    width: widths.progress,
    minWidth: 150,
    resizable: true,
    render: (task) => h(TaskProgressCell, { task }),
    },
    {
    key: "size",
    title: "已下载 / 总大小",
    width: widths.size,
    minWidth: 150,
    resizable: true,
    render: (task) => formatSizePair(task),
    },
    {
    key: "speed",
    title: "速度",
    width: widths.speed,
    minWidth: 110,
    resizable: true,
    render: (task) => `${formatSize(task.downloadSpeed)}/s`,
    },
    {
    key: "eta",
    title: "剩余时间",
    width: widths.eta,
    minWidth: 100,
    resizable: true,
    render: (task) => formatEta(task),
    },
    {
    key: "actions",
    title: "操作",
    width: widths.actions,
    minWidth: 240,
    resizable: false,
    fixed: "right",
    render: (task) => h(TaskActions, { task }),
    },
  ];
}

function handleColumnsUpdate(nextColumns: DataTableColumns<DownloadTask>) {
  columns.value = nextColumns;
  void saveUiPreferences({
    taskTableColumnWidths: extractColumnWidths(nextColumns),
  });
}

function extractColumnWidths(nextColumns: DataTableColumns<DownloadTask>) {
  const widths: Record<string, number> = {};
  for (const column of nextColumns) {
    const dataColumn = column as { key?: unknown; width?: unknown };
    if (!dataColumn.key || typeof dataColumn.width !== "number") {
      continue;
    }
    widths[String(dataColumn.key)] = dataColumn.width;
  }
  return widths;
}

function normalizeColumnWidth(width: unknown) {
  return typeof width === "number" ? width : 0;
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
</script>

<template>
  <section class="task-table-shell">
    <NDataTable
      class="task-data-table"
      :columns="columns"
      :data="props.tasks"
      :bordered="false"
      :single-line="false"
      :row-key="(task: DownloadTask) => task.id"
      :scroll-x="scrollX"
      flex-height
      @update:columns="handleColumnsUpdate"
    />
  </section>
</template>

<style scoped>
.task-table-shell {
  min-height: 0;
  height: 100%;
  padding: 22px;
}

.task-data-table {
  height: 100%;
}

:deep(.task-name-cell) {
  min-width: 0;
  display: grid;
  gap: 5px;
}

:deep(.task-name-cell strong) {
  overflow: hidden;
  color: #f1f6f1;
  text-overflow: ellipsis;
  white-space: nowrap;
}

:deep(.task-name-cell small) {
  overflow: hidden;
  color: #8e9a91;
  text-overflow: ellipsis;
  white-space: nowrap;
}

:deep(.task-name-cell .task-error-detail) {
  color: #ff9b9b;
}
</style>
