<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { NButton, NCard, NCollapse, NCollapseItem, NModal, NSpace, useMessage } from "naive-ui";
import AppConfirmDialog from "../../../components/ui/AppConfirmDialog.vue";
import { copyTextToClipboard } from "../../../app/utils/clipboard";
import { useI18n } from "../../../i18n";
import type { DownloadTask } from "../../../types/tasks";
import type { TaskActionDetails, TaskActionLabels, TaskFileActionView } from "./taskActionViewModel";
import TaskProgressBar from "./TaskProgressBar.vue";
import TaskProxyToggle from "./TaskProxyToggle.vue";
import {
  deriveTaskDisplayStatus,
  formatTaskEta,
  formatTaskProgress,
  formatTaskSizePair,
  formatTaskSpeed,
  formatTaskStatusLabel,
  getTaskProgressPercentage,
  getTaskSourceType,
} from "../utils/taskFormat";

const props = withDefaults(defineProps<{
  show: boolean;
  details: TaskActionDetails;
  closeLabel: string;
  task: DownloadTask;
  isOperating: boolean;
  isActionDisabled: boolean;
  fileActions?: TaskFileActionView;
  labels: TaskActionLabels;
  showProxy?: boolean;
}>(), {
  showProxy: true,
});

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
const detailItems = computed(() => ({
  download: props.details.items.filter((item) => item.section !== "file"),
  file: props.details.items.filter((item) => item.section === "file"),
}));
const displayStatus = computed(() => deriveTaskDisplayStatus(props.task));
const statusLabel = computed(() => formatTaskStatusLabel(props.task));
const sourceType = computed(() => getTaskSourceType(props.task));
const isBtTask = computed(() => sourceType.value !== "url");
const sourceLabel = computed(() => t(`task.source.${sourceType.value}` as const));
const categoryLabel = computed(() => props.task.category || t("common.notAvailable"));
const formattedSpeed = computed(() => formatTaskSpeed(props.task));
const progressPercentage = computed(() => getTaskProgressPercentage(props.task));
const progressTone = computed(() => progressPercentage.value === null
  ? "empty"
  : props.task.status === "complete" ? "complete" : "default");
const titleId = computed(() => `task-detail-title-${props.task.id}`);

async function copyPath(path: string) {
  const result = await copyTextToClipboard(path);
  if (result.copied) {
    message.success(props.labels.copied);
  } else {
    message.warning(props.labels.copyFailed);
  }
}

watch(
  () => props.show,
  (show) => {
    if (!show) pendingProxyEnabled.value = null;
  },
);

function requestProxyUpdate(enabled: boolean) {
  if (isBtTask.value || enabled === props.task.useProxy || props.isActionDisabled) return;
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
    <NCard
      class="task-detail-card app-dialog app-dialog--fixed-body"
      content-class="task-detail-card-content"
      role="dialog"
      aria-modal="true"
      :aria-labelledby="titleId"
    >
      <template #header>
        <div class="task-detail-header">
          <p class="task-detail-eyebrow">{{ details.title }}</p>
          <div class="task-detail-header-kicker">
            <span class="task-detail-status" :class="`task-detail-status--${displayStatus}`">{{ statusLabel }}</span>
            <span class="task-detail-source">{{ sourceLabel }}</span>
          </div>
          <h2 :id="titleId" class="task-detail-title">{{ props.task.fileName }}</h2>
        </div>
      </template>
      <template #header-extra>
        <NButton quaternary circle :aria-label="closeLabel" :title="closeLabel" @click="emit('update:show', false)">×</NButton>
      </template>

      <section class="task-detail-summary" data-test="task-detail-summary">
        <div class="task-detail-summary-progress">
          <div class="task-detail-summary-progress-heading">
            <span>{{ t("task.detail.progress") }}</span>
            <strong>{{ formatTaskProgress(props.task) }}</strong>
          </div>
          <TaskProgressBar
            :percentage="progressPercentage"
            variant="card"
            :tone="progressTone"
            :aria-label="`${t('task.detail.progress')} ${props.task.fileName}`"
            :aria-valuetext="progressPercentage === null ? t('task.progress.unknown') : formatTaskProgress(props.task)"
          />
          <p>{{ formatTaskSizePair(props.task) }}</p>
        </div>
        <div class="task-detail-summary-metrics">
          <div class="task-detail-summary-metric">
            <span>{{ t("task.detail.speed") }}</span>
            <strong>{{ formattedSpeed }}</strong>
          </div>
          <div class="task-detail-summary-metric">
            <span>{{ t("task.table.eta") }}</span>
            <strong>{{ formatTaskEta(props.task) }}</strong>
          </div>
          <div class="task-detail-summary-metric">
            <span>{{ t("task.table.category") }}</span>
            <strong>{{ categoryLabel }}</strong>
          </div>
        </div>
      </section>

      <section v-if="detailItems.download.length" class="task-detail-section" data-test="task-detail-download-section">
        <h3>{{ t("task.detail.downloadInfo") }}</h3>
        <dl class="task-detail-list">
          <div v-for="item in detailItems.download" :key="item.label" class="task-detail-row" data-test="detail-item">
            <dt>{{ item.label }}</dt>
            <dd>{{ item.value }}</dd>
          </div>
        </dl>
      </section>

      <section v-if="detailItems.file.length || props.fileActions" class="task-detail-section" data-test="task-detail-file-section">
        <h3>{{ t("task.detail.fileInfo") }}</h3>
        <dl v-if="detailItems.file.length" class="task-detail-list">
          <div v-for="item in detailItems.file" :key="item.label" class="task-detail-row" data-test="detail-item">
            <dt>{{ item.label }}</dt>
            <dd>{{ item.value }}</dd>
          </div>
        </dl>
        <NSpace v-if="props.fileActions?.hostSupported" class="task-file-actions" :size="8" wrap>
          <NButton
            v-if="props.fileActions.context?.actions.openFilePath"
            size="small"
            secondary
            :loading="props.fileActions.loading"
            :disabled="props.isActionDisabled || props.fileActions.loading"
            @click="emit('openFile')"
          >
            {{ props.labels.openFile }}
          </NButton>
          <NButton
            v-if="props.fileActions.context?.actions.detailPaths.length"
            size="small"
            secondary
            :loading="props.fileActions.loading"
            :disabled="props.isActionDisabled || props.fileActions.loading"
            @click="emit('showFileDetails')"
          >
            {{ props.labels.fileDetails }}
          </NButton>
        </NSpace>
        <p v-else-if="props.fileActions" class="task-file-host-hint">{{ props.labels.hostOnly }}</p>
      </section>

      <section v-if="props.showProxy !== false && !isBtTask" class="task-detail-section" data-test="task-detail-settings-section">
        <h3>{{ t("task.detail.taskSettings") }}</h3>
        <TaskProxyToggle
          class="task-detail-proxy"
          :value="props.task.useProxy"
          :disabled="props.isActionDisabled"
          :loading="props.isOperating"
          @update:value="requestProxyUpdate"
        />
      </section>

      <section
        v-if="props.details.technicalItems?.length"
        class="task-detail-section"
        data-test="task-detail-technical-section"
      >
        <NCollapse class="task-technical-info" :default-expanded-names="[]">
          <NCollapseItem :title="props.labels.technicalInfo" name="technical-info">
            <dl class="task-detail-list task-detail-list--technical">
              <div v-for="item in props.details.technicalItems" :key="item.label" class="task-detail-row" data-test="detail-item">
                <dt>{{ item.label }}</dt>
                <dd>
                  <NSpace align="center" justify="space-between" :size="8" wrap>
                    <span class="task-technical-path">{{ item.value }}</span>
                    <NButton size="tiny" secondary @click="copyPath(item.value)">{{ props.labels.copyPath }}</NButton>
                  </NSpace>
                </dd>
              </div>
            </dl>
          </NCollapseItem>
        </NCollapse>
      </section>

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
