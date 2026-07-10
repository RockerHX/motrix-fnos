<script setup lang="ts">
import { watch, ref } from "vue";
import AppIcon from "../../../components/AppIcon.vue";
import { NButton, NCard, NCheckbox, NDescriptions, NDescriptionsItem, NModal, NSpace } from "naive-ui";
import type {
  TaskActionConfirmTexts,
  TaskActionDetails,
  TaskActionLabels,
  TaskActionPermissions,
  TaskActionState,
} from "./taskActionViewModel";

const props = withDefaults(
  defineProps<{
    compact?: boolean;
    variant?: "text" | "icon-pill";
    state: TaskActionState;
    permissions: TaskActionPermissions;
    labels: TaskActionLabels;
    details: TaskActionDetails;
    confirmTexts: TaskActionConfirmTexts;
  }>(),
  {
    compact: false,
    variant: "text",
  },
);

const emit = defineEmits<{
  pause: [];
  resume: [];
  confirmFiles: [];
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
  () => props.state.isRuntimeExiting,
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
  <div v-if="props.variant === 'icon-pill'" class="icon-pill-actions" role="toolbar" :aria-label="props.labels.details">
    <button
      type="button"
      class="icon-action"
      :title="props.labels.details"
      :aria-label="props.labels.details"
      :disabled="props.state.isActionDisabled"
      @click="showDetails = true"
    >
      <AppIcon name="info" :size="14" />
    </button>
    <button
      v-if="props.permissions.canPause"
      type="button"
      class="icon-action"
      :class="{ loading: props.state.isOperating }"
      :title="props.labels.pause"
      :aria-label="props.labels.pause"
      :aria-busy="props.state.isOperating ? 'true' : undefined"
      :disabled="props.state.isActionDisabled"
      @click="emit('pause')"
    >
      <AppIcon name="pause" :size="14" />
    </button>
    <button
      v-if="props.permissions.canResume"
      type="button"
      class="icon-action"
      :class="{ loading: props.state.isOperating }"
      :title="props.labels.resume"
      :aria-label="props.labels.resume"
      :aria-busy="props.state.isOperating ? 'true' : undefined"
      :disabled="props.state.isActionDisabled"
      @click="emit('resume')"
    >
      <AppIcon name="play" :size="14" />
    </button>
    <button
      v-if="props.permissions.canConfirmFiles"
      type="button"
      class="icon-action icon-action--primary"
      :class="{ loading: props.state.isOperating }"
      :title="props.labels.confirmFiles"
      :aria-label="props.labels.confirmFiles"
      :aria-busy="props.state.isOperating ? 'true' : undefined"
      :disabled="props.state.isActionDisabled"
      @click="emit('confirmFiles')"
    >
      <AppIcon name="confirm" :size="14" />
    </button>
    <button
      v-if="props.permissions.canRedownload"
      type="button"
      class="icon-action"
      :title="props.labels.redownload"
      :aria-label="props.labels.redownload"
      :disabled="props.state.isActionDisabled"
      @click="showRedownloadConfirm = true"
    >
      <AppIcon name="redownload" :size="14" />
    </button>
    <button
      v-if="props.permissions.canDelete"
      type="button"
      class="icon-action icon-action--danger"
      :title="props.labels.delete"
      :aria-label="props.labels.delete"
      :disabled="props.state.isActionDisabled"
      @click="openDeleteConfirm"
    >
      <AppIcon name="delete" :size="14" />
    </button>
    <button
      v-if="props.permissions.canPermanentDelete"
      type="button"
      class="icon-action icon-action--danger"
      :class="{ loading: props.state.isOperating }"
      :title="props.labels.permanentDelete"
      :aria-label="props.labels.permanentDelete"
      :aria-busy="props.state.isOperating ? 'true' : undefined"
      :disabled="props.state.isActionDisabled"
      @click="showPermanentDeleteConfirm = true"
    >
      <AppIcon name="permanentDelete" :size="14" />
    </button>
  </div>
  <div v-else-if="props.compact" class="compact-actions">
    <NButton
      size="small"
      secondary
      :title="props.labels.details"
      :aria-label="props.labels.details"
      :disabled="props.state.isActionDisabled"
      @click="showDetails = true"
    >
      {{ props.labels.details }}
    </NButton>
    <NButton
      v-if="props.permissions.canPause"
      size="small"
      secondary
      :title="props.labels.pause"
      :aria-label="props.labels.pause"
      :loading="props.state.isOperating"
      :disabled="props.state.isActionDisabled"
      @click="emit('pause')"
    >
      {{ props.labels.pause }}
    </NButton>
    <NButton
      v-if="props.permissions.canResume"
      size="small"
      secondary
      :title="props.labels.resume"
      :aria-label="props.labels.resume"
      :loading="props.state.isOperating"
      :disabled="props.state.isActionDisabled"
      @click="emit('resume')"
    >
      {{ props.labels.resume }}
    </NButton>
    <NButton
      v-if="props.permissions.canConfirmFiles"
      size="small"
      secondary
      type="primary"
      :title="props.labels.confirmFiles"
      :aria-label="props.labels.confirmFiles"
      :loading="props.state.isOperating"
      :disabled="props.state.isActionDisabled"
      @click="emit('confirmFiles')"
    >
      {{ props.labels.confirmFiles }}
    </NButton>
    <NButton
      v-if="props.permissions.canRedownload"
      size="small"
      secondary
      :title="props.labels.redownload"
      :aria-label="props.labels.redownload"
      :disabled="props.state.isActionDisabled"
      @click="showRedownloadConfirm = true"
    >
      {{ props.labels.redownload }}
    </NButton>
    <NButton
      v-if="props.permissions.canDelete"
      size="small"
      secondary
      type="error"
      :title="props.labels.delete"
      :aria-label="props.labels.delete"
      :disabled="props.state.isActionDisabled"
      @click="openDeleteConfirm"
    >
      {{ props.labels.delete }}
    </NButton>
    <NButton
      v-if="props.permissions.canPermanentDelete"
      size="small"
      secondary
      type="error"
      :title="props.labels.permanentDelete"
      :aria-label="props.labels.permanentDelete"
      :loading="props.state.isOperating"
      :disabled="props.state.isActionDisabled"
      @click="showPermanentDeleteConfirm = true"
    >
      {{ props.labels.permanentDelete }}
    </NButton>
  </div>
  <NSpace v-else :size="6" wrap>
    <NButton size="small" secondary :disabled="props.state.isActionDisabled" @click="showDetails = true">
      {{ props.labels.details }}
    </NButton>
    <NButton
      v-if="props.permissions.canPause"
      size="small"
      secondary
      :loading="props.state.isOperating"
      :disabled="props.state.isActionDisabled"
      @click="emit('pause')"
    >
      {{ props.labels.pause }}
    </NButton>
    <NButton
      v-if="props.permissions.canResume"
      size="small"
      secondary
      :loading="props.state.isOperating"
      :disabled="props.state.isActionDisabled"
      @click="emit('resume')"
    >
      {{ props.labels.resume }}
    </NButton>
    <NButton
      v-if="props.permissions.canConfirmFiles"
      size="small"
      secondary
      type="primary"
      :loading="props.state.isOperating"
      :disabled="props.state.isActionDisabled"
      @click="emit('confirmFiles')"
    >
      {{ props.labels.confirmFiles }}
    </NButton>
    <NButton
      v-if="props.permissions.canRedownload"
      size="small"
      secondary
      :disabled="props.state.isActionDisabled"
      @click="showRedownloadConfirm = true"
    >
      {{ props.labels.redownload }}
    </NButton>
    <NButton
      v-if="props.permissions.canDelete"
      size="small"
      secondary
      type="error"
      :disabled="props.state.isActionDisabled"
      @click="openDeleteConfirm"
    >
      {{ props.labels.delete }}
    </NButton>
    <NButton
      v-if="props.permissions.canPermanentDelete"
      size="small"
      secondary
      type="error"
      :loading="props.state.isOperating"
      :disabled="props.state.isActionDisabled"
      @click="showPermanentDeleteConfirm = true"
    >
      {{ props.labels.permanentDelete }}
    </NButton>
  </NSpace>

  <NModal v-model:show="showDetails">
    <NCard class="task-detail-card app-dialog" role="dialog" aria-modal="true" :title="props.details.title">
      <NDescriptions :column="1" label-placement="left" bordered>
        <NDescriptionsItem v-for="item in props.details.items" :key="item.label" :label="item.label">
          {{ item.value }}
        </NDescriptionsItem>
      </NDescriptions>

      <template #footer>
        <NSpace justify="end">
          <NButton @click="showDetails = false">{{ props.labels.close }}</NButton>
        </NSpace>
      </template>
    </NCard>
  </NModal>

  <NModal v-model:show="showRedownloadConfirm" :mask-closable="!props.state.isOperating">
    <NCard class="redownload-confirm-card app-dialog" role="dialog" aria-modal="true" :title="props.confirmTexts.redownloadTitle">
      <p class="delete-confirm-text">
        {{ props.confirmTexts.redownloadConfirmText }}
      </p>

      <template #footer>
        <NSpace justify="end">
          <NButton :disabled="props.state.isActionDisabled" @click="showRedownloadConfirm = false">{{ props.labels.cancel }}</NButton>
          <NButton
            type="primary"
            :loading="props.state.isOperating"
            :disabled="props.state.isActionDisabled"
            @click="emit('confirmRedownload')"
          >
            {{ props.labels.redownload }}
          </NButton>
        </NSpace>
      </template>
    </NCard>
  </NModal>

  <NModal v-model:show="showDeleteConfirm" :mask-closable="!props.state.isOperating">
    <NCard class="delete-confirm-card app-dialog" role="dialog" aria-modal="true" :title="props.confirmTexts.deleteTitle">
      <p class="delete-confirm-text">{{ props.confirmTexts.deleteConfirmText }}</p>
      <NCheckbox v-model:checked="deleteFiles">{{ props.confirmTexts.deleteFilesLabel }}</NCheckbox>

      <template #footer>
        <NSpace justify="end">
          <NButton :disabled="props.state.isActionDisabled" @click="showDeleteConfirm = false">{{ props.labels.cancel }}</NButton>
          <NButton type="error" :loading="props.state.isOperating" :disabled="props.state.isActionDisabled" @click="emitDeleteConfirm">
            {{ props.labels.delete }}
          </NButton>
        </NSpace>
      </template>
    </NCard>
  </NModal>

  <NModal v-model:show="showPermanentDeleteConfirm" :mask-closable="!props.state.isOperating">
    <NCard class="permanent-delete-confirm-card app-dialog" role="dialog" aria-modal="true" :title="props.confirmTexts.permanentDeleteTitle">
      <p class="delete-confirm-text">
        {{ props.confirmTexts.permanentDeleteConfirmText }}
      </p>

      <template #footer>
        <NSpace justify="end">
          <NButton :disabled="props.state.isActionDisabled" @click="showPermanentDeleteConfirm = false">{{ props.labels.cancel }}</NButton>
          <NButton
            type="error"
            :loading="props.state.isOperating"
            :disabled="props.state.isActionDisabled"
            @click="emit('confirmPermanentDelete')"
          >
            {{ props.labels.permanentDelete }}
          </NButton>
        </NSpace>
      </template>
    </NCard>
  </NModal>
</template>

<style scoped>
.icon-pill-actions {
  display: inline-flex;
  align-items: center;
  justify-content: flex-end;
  gap: 3px;
  max-width: 100%;
  padding: 1px 4px;
  border: 1px solid color-mix(in srgb, var(--app-color-border-subtle) 76%, transparent);
  border-radius: var(--app-radius-pill);
  background: rgba(255, 255, 255, 0.012);
}

.icon-action {
  width: 26px;
  height: 26px;
  display: inline-grid;
  place-items: center;
  border: 0;
  border-radius: var(--app-radius-pill);
  padding: 0;
  color: var(--app-text-dim);
  background: transparent;
  font: inherit;
  font-size: 14px;
  line-height: 1;
  cursor: pointer;
  opacity: 0.62;
}

.icon-action:hover,
.icon-action:focus-visible {
  color: var(--app-text-strong);
  background: var(--app-color-card-overlay-subtle);
  opacity: 1;
  outline: none;
}

.icon-action:disabled {
  cursor: not-allowed;
  opacity: 0.38;
}

.icon-action.loading {
  opacity: 0.62;
}

.icon-action--primary {
  color: color-mix(in srgb, var(--app-text-accent) 82%, var(--app-text-muted));
}

.icon-action--danger {
  color: color-mix(in srgb, var(--app-text-danger) 78%, var(--app-text-muted));
}

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
