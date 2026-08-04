<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { NButton, NCard, NDescriptions, NDescriptionsItem, NModal, NSpace } from "naive-ui";
import AppConfirmDialog from "../../../components/ui/AppConfirmDialog.vue";
import { useI18n } from "../../../i18n";
import type { DownloadTask } from "../../../types/tasks";
import type { TaskActionDetails } from "./taskActionViewModel";
import TaskProxyToggle from "./TaskProxyToggle.vue";

const props = defineProps<{
  show: boolean;
  details: TaskActionDetails;
  closeLabel: string;
  task: DownloadTask;
  isOperating: boolean;
  isActionDisabled: boolean;
}>();

const emit = defineEmits<{
  "update:show": [show: boolean];
  updateProxy: [enabled: boolean];
}>();

const { t } = useI18n();
const pendingProxyEnabled = ref<boolean | null>(null);
const showProxyConfirm = computed(() => pendingProxyEnabled.value !== null);

watch(
  () => props.show,
  (show) => {
    if (!show) pendingProxyEnabled.value = null;
  },
);

function requestProxyUpdate(enabled: boolean) {
  if (enabled === props.task.useProxy || props.isActionDisabled) return;
  if (props.task.status === "active") {
    pendingProxyEnabled.value = enabled;
    return;
  }
  emit("updateProxy", enabled);
}

function updateProxyConfirm(show: boolean) {
  if (!show) pendingProxyEnabled.value = null;
}

function confirmProxyUpdate() {
  const enabled = pendingProxyEnabled.value;
  if (enabled === null) return;
  pendingProxyEnabled.value = null;
  emit("updateProxy", enabled);
}
</script>

<template>
  <NModal :show="show" @update:show="emit('update:show', $event)">
    <NCard class="task-detail-card app-dialog" role="dialog" aria-modal="true" :title="details.title">
      <NDescriptions :column="1" label-placement="left" bordered>
        <NDescriptionsItem v-for="item in details.items" :key="item.label" :label="item.label">
          {{ item.value }}
        </NDescriptionsItem>
      </NDescriptions>

      <TaskProxyToggle
        class="task-detail-proxy"
        :value="props.task.useProxy"
        :disabled="props.isActionDisabled"
        :loading="props.isOperating"
        @update:value="requestProxyUpdate"
      />

      <template #footer>
        <NSpace justify="end">
          <NButton @click="emit('update:show', false)">{{ closeLabel }}</NButton>
        </NSpace>
      </template>
    </NCard>
  </NModal>

  <AppConfirmDialog
    :show="showProxyConfirm"
    :title="t('task.proxy.reconnectTitle')"
    :confirm-text="t('task.proxy.reconnectConfirm')"
    :mask-closable="!props.isOperating"
    :loading="props.isOperating"
    :disabled="props.isActionDisabled"
    @update:show="updateProxyConfirm"
    @confirm="confirmProxyUpdate"
  >
    <template #confirm-label>{{ t("task.proxy.change") }}</template>
  </AppConfirmDialog>
</template>

<style scoped src="./TaskDetailsDialog.css"></style>
