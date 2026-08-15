<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { NButton, NCard, NCollapse, NCollapseItem, NDescriptions, NDescriptionsItem, NModal, NSpace, useMessage } from "naive-ui";
import AppConfirmDialog from "../../../components/ui/AppConfirmDialog.vue";
import { copyTextToClipboard } from "../../../app/utils/clipboard";
import { useI18n } from "../../../i18n";
import type { DownloadTask } from "../../../types/tasks";
import type { TaskActionDetails, TaskActionLabels, TaskFileActionView } from "./taskActionViewModel";
import TaskProxyToggle from "./TaskProxyToggle.vue";

const props = defineProps<{
  show: boolean;
  details: TaskActionDetails;
  closeLabel: string;
  task: DownloadTask;
  isOperating: boolean;
  isActionDisabled: boolean;
  fileActions?: TaskFileActionView;
  labels?: TaskActionLabels;
}>();

const emit = defineEmits<{
  "update:show": [show: boolean];
  updateProxy: [enabled: boolean];
  openFile: [];
  showFileDetails: [];
}>();

const { t } = useI18n();
const message = useMessage();
const pendingProxyEnabled = ref<boolean | null>(null);
const showProxyConfirm = computed(() => pendingProxyEnabled.value !== null);
const actionLabels = computed(() =>
  props.labels ?? {
    details: t("task.actions.details"),
    pause: t("task.actions.pause"),
    resume: t("task.actions.resume"),
    confirmFiles: t("task.actions.confirmFiles"),
    redownload: t("task.actions.redownload"),
    delete: t("task.actions.delete"),
    restore: t("task.actions.restore"),
    permanentDelete: t("task.actions.permanentDelete"),
    cancel: t("common.cancel"),
    close: t("common.close"),
    openFileManager: t("task.actions.openFileManager"),
    openFile: t("task.actions.openFile"),
    fileDetails: t("task.actions.fileDetails"),
    hostOnly: t("task.fileOperations.hostOnly"),
    technicalInfo: t("task.fileOperations.technicalInfo"),
    copyPath: t("common.copy"),
    copied: t("common.copied"),
    copyFailed: t("task.fileOperations.copyFailed"),
  } satisfies TaskActionLabels,
);

async function copyPath(path: string) {
  const result = await copyTextToClipboard(path);
  if (result.copied) {
    message.success(actionLabels.value.copied);
  } else {
    message.warning(actionLabels.value.copyFailed);
  }
}

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

      <NSpace v-if="props.fileActions?.hostSupported" class="task-file-actions" :size="8" wrap>
        <NButton
          v-if="props.fileActions.context?.actions.openFilePath"
          size="small"
          secondary
          :loading="props.fileActions.loading"
          :disabled="props.isActionDisabled || props.fileActions.loading"
          @click="emit('openFile')"
        >
          {{ actionLabels.openFile }}
        </NButton>
        <NButton
          v-if="props.fileActions.context?.actions.detailPaths.length"
          size="small"
          secondary
          :loading="props.fileActions.loading"
          :disabled="props.isActionDisabled || props.fileActions.loading"
          @click="emit('showFileDetails')"
        >
          {{ actionLabels.fileDetails }}
        </NButton>
      </NSpace>
      <p v-else class="task-file-host-hint">{{ actionLabels.hostOnly }}</p>

      <NCollapse v-if="props.details.technicalItems?.length" class="task-technical-info">
        <NCollapseItem :title="actionLabels.technicalInfo" name="technical-info">
          <NDescriptions :column="1" label-placement="left" bordered>
            <NDescriptionsItem v-for="item in props.details.technicalItems" :key="item.label" :label="item.label">
              <NSpace align="center" justify="space-between" :size="8" wrap>
                <span class="task-technical-path">{{ item.value }}</span>
                <NButton size="tiny" secondary @click="copyPath(item.value)">{{ actionLabels.copyPath }}</NButton>
              </NSpace>
            </NDescriptionsItem>
          </NDescriptions>
        </NCollapseItem>
      </NCollapse>

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
