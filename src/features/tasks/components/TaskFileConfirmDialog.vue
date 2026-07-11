<script setup lang="ts">
import { computed, h, ref, watch } from "vue";
import { NButton, NDataTable } from "naive-ui";
import type { DataTableColumns, DataTableRowKey } from "naive-ui";
import AppDialog from "../../../components/ui/AppDialog.vue";
import AppDialogActions from "../../../components/ui/AppDialogActions.vue";
import { useI18n } from "../../../i18n";
import { formatTaskSize } from "../utils/taskFormat";
import type { DownloadTask, DownloadTaskFile } from "../../../types/tasks";

const props = defineProps<{
  show: boolean;
  task: DownloadTask | null;
  isLoading?: boolean;
}>();

const emit = defineEmits<{
  "update:show": [show: boolean];
  confirm: [selectedFileIndexes: number[]];
}>();

const { t } = useI18n();
const selectedIndexes = ref<number[]>([]);

const files = computed(() => props.task?.files ?? []);
const selectedCount = computed(() => selectedIndexes.value.length);
const canConfirm = computed(() => selectedCount.value > 0 && !props.isLoading);
const fileColumns = computed<DataTableColumns<DownloadTaskFile>>(() => [
  {
    type: "selection",
    disabled: () => Boolean(props.isLoading),
  },
  {
    title: t("task.fileConfirm.name"),
    key: "name",
  },
  {
    title: t("task.fileConfirm.path"),
    key: "path",
    render: (file) => h("span", { class: "file-confirm-path" }, file.path),
  },
  {
    title: t("task.fileConfirm.size"),
    key: "size",
    render: (file) => formatTaskSize(file.length),
  },
]);

watch(
  () => [props.show, props.task?.id, files.value.map((file) => file.index).join(",")],
  () => {
    if (!props.show) {
      return;
    }
    selectedIndexes.value = files.value.map((file) => file.index);
  },
  { immediate: true },
);

function close() {
  if (!props.isLoading) {
    emit("update:show", false);
  }
}

function getRowKey(file: DownloadTaskFile) {
  return file.index;
}

function updateSelectedIndexes(keys: DataTableRowKey[]) {
  if (props.isLoading) {
    return;
  }

  selectedIndexes.value = keys
    .map((key) => Number(key))
    .filter((key) => Number.isFinite(key))
    .sort((a, b) => a - b);
}

function confirm() {
  if (!canConfirm.value) {
    return;
  }
  emit("confirm", [...selectedIndexes.value].sort((a, b) => a - b));
}
</script>

<template>
  <AppDialog
    :show="props.show"
    :mask-closable="!props.isLoading"
    :close-disabled="props.isLoading"
    :title="t('task.fileConfirm.title')"
    width="760px"
    card-class="file-confirm-card"
    @update:show="emit('update:show', $event)"
  >
    <p class="file-confirm-description">{{ t("task.fileConfirm.description") }}</p>
    <div class="file-confirm-table-wrap">
      <NDataTable
        :columns="fileColumns"
        :data="files"
        :row-key="getRowKey"
        :checked-row-keys="selectedIndexes"
        :max-height="420"
        size="small"
        @update:checked-row-keys="updateSelectedIndexes"
      />
    </div>
    <p v-if="selectedCount === 0" class="file-confirm-error">{{ t("task.fileConfirm.selectAtLeastOne") }}</p>

    <template #footer>
      <AppDialogActions>
        <NButton :disabled="props.isLoading" @click="close">{{ t("common.cancel") }}</NButton>
        <NButton type="primary" :loading="props.isLoading" :disabled="!canConfirm" @click="confirm">
          {{ t("task.fileConfirm.start") }}
        </NButton>
      </AppDialogActions>
    </template>
  </AppDialog>
</template>

<style scoped>
.file-confirm-description {
  margin: 0 0 14px;
  color: var(--app-text-secondary);
}

.file-confirm-table-wrap {
  max-height: min(52vh, 420px);
  overflow: auto;
}

.file-confirm-path {
  word-break: break-all;
  color: var(--app-text-secondary);
}

.file-confirm-error {
  margin: 12px 0 0;
  color: var(--app-danger-color);
}
</style>
