<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { NButton, NCard, NCheckbox, NModal, NSpace } from "naive-ui";
import { useI18n } from "../../../i18n";
import { formatTaskSize } from "../utils/taskFormat";
import type { DownloadTask } from "../../../types/tasks";

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
  emit("update:show", false);
}

function toggleFile(index: number, checked: boolean) {
  if (checked) {
    selectedIndexes.value = [...new Set([...selectedIndexes.value, index])].sort((a, b) => a - b);
    return;
  }
  selectedIndexes.value = selectedIndexes.value.filter((item) => item !== index);
}

function confirm() {
  if (!canConfirm.value) {
    return;
  }
  emit("confirm", [...selectedIndexes.value].sort((a, b) => a - b));
}
</script>

<template>
  <NModal :show="props.show" :mask-closable="!props.isLoading" @update:show="emit('update:show', $event)">
    <NCard class="file-confirm-card app-dialog" role="dialog" aria-modal="true" :title="t('task.fileConfirm.title')">
      <p class="file-confirm-description">{{ t("task.fileConfirm.description") }}</p>
      <div class="file-confirm-table-wrap">
        <table class="file-confirm-table">
          <thead>
            <tr>
              <th class="file-confirm-check">{{ t("task.fileConfirm.select") }}</th>
              <th>{{ t("task.fileConfirm.name") }}</th>
              <th>{{ t("task.fileConfirm.path") }}</th>
              <th>{{ t("task.fileConfirm.size") }}</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="file in files" :key="file.index">
              <td class="file-confirm-check">
                <NCheckbox
                  :checked="selectedIndexes.includes(file.index)"
                  :disabled="props.isLoading"
                  @update:checked="toggleFile(file.index, Boolean($event))"
                />
              </td>
              <td>{{ file.name }}</td>
              <td class="file-confirm-path">{{ file.path }}</td>
              <td>{{ formatTaskSize(file.length) }}</td>
            </tr>
          </tbody>
        </table>
      </div>
      <p v-if="selectedCount === 0" class="file-confirm-error">{{ t("task.fileConfirm.selectAtLeastOne") }}</p>

      <template #footer>
        <NSpace justify="end">
          <NButton :disabled="props.isLoading" @click="close">{{ t("common.cancel") }}</NButton>
          <NButton type="primary" :loading="props.isLoading" :disabled="!canConfirm" @click="confirm">
            {{ t("task.fileConfirm.start") }}
          </NButton>
        </NSpace>
      </template>
    </NCard>
  </NModal>
</template>

<style scoped>
.file-confirm-card {
  --app-dialog-width: 760px;
  --app-dialog-mobile-margin: 16px;
}

.file-confirm-description {
  margin: 0 0 14px;
  color: var(--app-text-secondary);
}

.file-confirm-table-wrap {
  max-height: min(52vh, 420px);
  overflow: auto;
  border: 1px solid var(--app-border-color);
  border-radius: var(--app-radius-sm);
}

.file-confirm-table {
  width: 100%;
  border-collapse: collapse;
  font-size: 13px;
}

.file-confirm-table th,
.file-confirm-table td {
  padding: 10px;
  border-bottom: 1px solid var(--app-border-color);
  text-align: left;
  vertical-align: top;
}

.file-confirm-table tbody tr:last-child td {
  border-bottom: 0;
}

.file-confirm-check {
  width: 64px;
  text-align: center;
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
