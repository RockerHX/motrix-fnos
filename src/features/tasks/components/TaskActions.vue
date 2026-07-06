<script setup lang="ts">
import { watch, ref } from "vue";
import { NButton, NCard, NCheckbox, NDescriptions, NDescriptionsItem, NModal, NSpace } from "naive-ui";

const props = defineProps<{
  compact?: boolean;
  isOperating: boolean;
  isActionDisabled: boolean;
  isRuntimeExiting: boolean;
  canPause: boolean;
  canResume: boolean;
  canRedownload: boolean;
  canDelete: boolean;
  canPermanentDelete: boolean;
  detailsLabel: string;
  pauseLabel: string;
  resumeLabel: string;
  redownloadLabel: string;
  deleteLabel: string;
  permanentDeleteLabel: string;
  cancelLabel: string;
  closeLabel: string;
  detailTitle: string;
  detailFileNameLabel: string;
  detailStatusLabel: string;
  detailProgressLabel: string;
  detailSizeLabel: string;
  detailSpeedLabel: string;
  detailSaveDirLabel: string;
  detailFilePathLabel: string;
  detailGidLabel: string;
  detailUrlLabel: string;
  detailCreatedAtLabel: string;
  detailUpdatedAtLabel: string;
  detailErrorReasonLabel: string;
  detailFileName: string;
  detailStatus: string;
  detailProgress: string;
  detailSize: string;
  detailSpeed: string;
  detailSaveDir: string;
  detailFilePath: string;
  detailGid: string;
  detailUrl: string;
  detailCreatedAt: string;
  detailUpdatedAt: string;
  detailErrorReason?: string;
  redownloadTitle: string;
  redownloadConfirmText: string;
  deleteTitle: string;
  deleteConfirmText: string;
  deleteFilesLabel: string;
  permanentDeleteTitle: string;
  permanentDeleteConfirmText: string;
}>();

const emit = defineEmits<{
  pause: [];
  resume: [];
  confirmRedownload: [];
  confirmDelete: [deleteFiles: boolean];
  confirmPermanentDelete: [];
}>();

const showDeleteConfirm = ref(false);
const showPermanentDeleteConfirm = ref(false);
const showRedownloadConfirm = ref(false);
const showDetails = ref(false);
const deleteFiles = ref(false);

watch(
  () => props.isRuntimeExiting,
  (isRuntimeExiting) => {
    if (!isRuntimeExiting) {
      return;
    }

    showDeleteConfirm.value = false;
    showPermanentDeleteConfirm.value = false;
    showRedownloadConfirm.value = false;
    showDetails.value = false;
    deleteFiles.value = false;
  },
);

function openDeleteConfirm() {
  deleteFiles.value = false;
  showDeleteConfirm.value = true;
}

function emitDeleteConfirm() {
  emit("confirmDelete", deleteFiles.value);
}
</script>

<template>
  <div v-if="props.compact" class="compact-actions">
    <NButton
      size="small"
      secondary
      :title="props.detailsLabel"
      :aria-label="props.detailsLabel"
      :disabled="props.isActionDisabled"
      @click="showDetails = true"
    >
      {{ props.detailsLabel }}
    </NButton>
    <NButton
      v-if="props.canPause"
      size="small"
      secondary
      :title="props.pauseLabel"
      :aria-label="props.pauseLabel"
      :loading="props.isOperating"
      :disabled="props.isActionDisabled"
      @click="emit('pause')"
    >
      {{ props.pauseLabel }}
    </NButton>
    <NButton
      v-if="props.canResume"
      size="small"
      secondary
      :title="props.resumeLabel"
      :aria-label="props.resumeLabel"
      :loading="props.isOperating"
      :disabled="props.isActionDisabled"
      @click="emit('resume')"
    >
      {{ props.resumeLabel }}
    </NButton>
    <NButton
      v-if="props.canRedownload"
      size="small"
      secondary
      :title="props.redownloadLabel"
      :aria-label="props.redownloadLabel"
      :disabled="props.isActionDisabled"
      @click="showRedownloadConfirm = true"
    >
      {{ props.redownloadLabel }}
    </NButton>
    <NButton
      v-if="props.canDelete"
      size="small"
      secondary
      type="error"
      :title="props.deleteLabel"
      :aria-label="props.deleteLabel"
      :disabled="props.isActionDisabled"
      @click="openDeleteConfirm"
    >
      {{ props.deleteLabel }}
    </NButton>
    <NButton
      v-if="props.canPermanentDelete"
      size="small"
      secondary
      type="error"
      :title="props.permanentDeleteLabel"
      :aria-label="props.permanentDeleteLabel"
      :loading="props.isOperating"
      :disabled="props.isActionDisabled"
      @click="showPermanentDeleteConfirm = true"
    >
      {{ props.permanentDeleteLabel }}
    </NButton>
  </div>
  <NSpace v-else :size="6" wrap>
    <NButton size="small" secondary :disabled="props.isActionDisabled" @click="showDetails = true">
      {{ props.detailsLabel }}
    </NButton>
    <NButton
      v-if="props.canPause"
      size="small"
      secondary
      :loading="props.isOperating"
      :disabled="props.isActionDisabled"
      @click="emit('pause')"
    >
      {{ props.pauseLabel }}
    </NButton>
    <NButton
      v-if="props.canResume"
      size="small"
      secondary
      :loading="props.isOperating"
      :disabled="props.isActionDisabled"
      @click="emit('resume')"
    >
      {{ props.resumeLabel }}
    </NButton>
    <NButton
      v-if="props.canRedownload"
      size="small"
      secondary
      :disabled="props.isActionDisabled"
      @click="showRedownloadConfirm = true"
    >
      {{ props.redownloadLabel }}
    </NButton>
    <NButton
      v-if="props.canDelete"
      size="small"
      secondary
      type="error"
      :disabled="props.isActionDisabled"
      @click="openDeleteConfirm"
    >
      {{ props.deleteLabel }}
    </NButton>
    <NButton
      v-if="props.canPermanentDelete"
      size="small"
      secondary
      type="error"
      :loading="props.isOperating"
      :disabled="props.isActionDisabled"
      @click="showPermanentDeleteConfirm = true"
    >
      {{ props.permanentDeleteLabel }}
    </NButton>
  </NSpace>

  <NModal v-model:show="showDetails">
    <NCard class="task-detail-card app-dialog" role="dialog" aria-modal="true" :title="props.detailTitle">
      <NDescriptions :column="1" label-placement="left" bordered>
        <NDescriptionsItem :label="props.detailFileNameLabel">{{ props.detailFileName }}</NDescriptionsItem>
        <NDescriptionsItem :label="props.detailStatusLabel">{{ props.detailStatus }}</NDescriptionsItem>
        <NDescriptionsItem :label="props.detailProgressLabel">{{ props.detailProgress }}</NDescriptionsItem>
        <NDescriptionsItem :label="props.detailSizeLabel">{{ props.detailSize }}</NDescriptionsItem>
        <NDescriptionsItem :label="props.detailSpeedLabel">{{ props.detailSpeed }}</NDescriptionsItem>
        <NDescriptionsItem :label="props.detailSaveDirLabel">{{ props.detailSaveDir }}</NDescriptionsItem>
        <NDescriptionsItem :label="props.detailFilePathLabel">{{ props.detailFilePath }}</NDescriptionsItem>
        <NDescriptionsItem :label="props.detailGidLabel">{{ props.detailGid }}</NDescriptionsItem>
        <NDescriptionsItem :label="props.detailUrlLabel">{{ props.detailUrl }}</NDescriptionsItem>
        <NDescriptionsItem :label="props.detailCreatedAtLabel">{{ props.detailCreatedAt }}</NDescriptionsItem>
        <NDescriptionsItem :label="props.detailUpdatedAtLabel">{{ props.detailUpdatedAt }}</NDescriptionsItem>
        <NDescriptionsItem v-if="props.detailErrorReason" :label="props.detailErrorReasonLabel">
          {{ props.detailErrorReason }}
        </NDescriptionsItem>
      </NDescriptions>

      <template #footer>
        <NSpace justify="end">
          <NButton @click="showDetails = false">{{ props.closeLabel }}</NButton>
        </NSpace>
      </template>
    </NCard>
  </NModal>

  <NModal v-model:show="showRedownloadConfirm" :mask-closable="!props.isOperating">
    <NCard class="redownload-confirm-card app-dialog" role="dialog" aria-modal="true" :title="props.redownloadTitle">
      <p class="delete-confirm-text">
        {{ props.redownloadConfirmText }}
      </p>

      <template #footer>
        <NSpace justify="end">
          <NButton :disabled="props.isActionDisabled" @click="showRedownloadConfirm = false">{{ props.cancelLabel }}</NButton>
          <NButton type="primary" :loading="props.isOperating" :disabled="props.isActionDisabled" @click="emit('confirmRedownload')">
            {{ props.redownloadLabel }}
          </NButton>
        </NSpace>
      </template>
    </NCard>
  </NModal>

  <NModal v-model:show="showDeleteConfirm" :mask-closable="!props.isOperating">
    <NCard class="delete-confirm-card app-dialog" role="dialog" aria-modal="true" :title="props.deleteTitle">
      <p class="delete-confirm-text">{{ props.deleteConfirmText }}</p>
      <NCheckbox v-model:checked="deleteFiles">{{ props.deleteFilesLabel }}</NCheckbox>

      <template #footer>
        <NSpace justify="end">
          <NButton :disabled="props.isActionDisabled" @click="showDeleteConfirm = false">{{ props.cancelLabel }}</NButton>
          <NButton type="error" :loading="props.isOperating" :disabled="props.isActionDisabled" @click="emitDeleteConfirm">
            {{ props.deleteLabel }}
          </NButton>
        </NSpace>
      </template>
    </NCard>
  </NModal>

  <NModal v-model:show="showPermanentDeleteConfirm" :mask-closable="!props.isOperating">
    <NCard class="permanent-delete-confirm-card app-dialog" role="dialog" aria-modal="true" :title="props.permanentDeleteTitle">
      <p class="delete-confirm-text">
        {{ props.permanentDeleteConfirmText }}
      </p>

      <template #footer>
        <NSpace justify="end">
          <NButton :disabled="props.isActionDisabled" @click="showPermanentDeleteConfirm = false">{{ props.cancelLabel }}</NButton>
          <NButton
            type="error"
            :loading="props.isOperating"
            :disabled="props.isActionDisabled"
            @click="emit('confirmPermanentDelete')"
          >
            {{ props.permanentDeleteLabel }}
          </NButton>
        </NSpace>
      </template>
    </NCard>
  </NModal>
</template>

<style scoped>
.compact-actions {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 8px;
}

.compact-actions :deep(.n-button) {
  width: 100%;
}

.delete-confirm-card,
.permanent-delete-confirm-card,
.redownload-confirm-card {
  --app-dialog-width: 420px;
  --app-dialog-mobile-margin: 24px;
}

.task-detail-card {
  --app-dialog-width: 720px;
  --app-dialog-mobile-margin: 24px;
}

.delete-confirm-text {
  margin: 0 0 14px;
  color: var(--app-text-secondary);
  word-break: break-word;
}

:deep(.n-descriptions-table-content__content) {
  word-break: break-all;
}

:deep(.n-descriptions-table-header) {
  word-break: break-word;
}

@media (max-width: 767px) {
  .compact-actions {
    gap: 10px;
  }

  .compact-actions :deep(.n-button) {
    min-height: var(--app-touch-target-min);
    border-radius: var(--app-radius-sm);
  }

  .delete-confirm-card,
  .permanent-delete-confirm-card,
  .redownload-confirm-card,
  .task-detail-card,
  .compact-actions {
    min-width: 0;
  }
}
</style>
