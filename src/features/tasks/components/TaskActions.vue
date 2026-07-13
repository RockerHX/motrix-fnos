<script setup lang="ts">
import { watch, ref } from "vue";
import AppIcon from "../../../components/AppIcon.vue";
import { NButton, NSpace } from "naive-ui";
import TaskDetailsDialog from "./TaskDetailsDialog.vue";
import TaskRedownloadConfirmDialog from "./TaskRedownloadConfirmDialog.vue";
import TaskDeleteConfirmDialog from "./TaskDeleteConfirmDialog.vue";
import TaskPermanentDeleteConfirmDialog from "./TaskPermanentDeleteConfirmDialog.vue";
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
  },
);

function openDeleteConfirm() {
  showDeleteConfirm.value = true;
}
</script>

<template>
  <div v-if="props.variant === 'icon-pill'" class="icon-pill-actions" role="toolbar" :aria-label="props.labels.details">
    <NButton
      quaternary
      circle
      size="tiny"
      class="icon-action"
      :title="props.labels.details"
      :aria-label="props.labels.details"
      :disabled="props.state.isActionDisabled"
      @click="showDetails = true"
    >
      <AppIcon name="info" :size="14" />
    </NButton>
    <NButton
      v-if="props.permissions.canPause"
      quaternary
      circle
      size="tiny"
      class="icon-action"
      :title="props.labels.pause"
      :aria-label="props.labels.pause"
      :aria-busy="props.state.isOperating ? 'true' : undefined"
      :loading="props.state.isOperating"
      :disabled="props.state.isActionDisabled"
      @click="emit('pause')"
    >
      <AppIcon v-if="!props.state.isOperating" name="pause" :size="14" />
    </NButton>
    <NButton
      v-if="props.permissions.canResume"
      quaternary
      circle
      size="tiny"
      class="icon-action"
      :title="props.labels.resume"
      :aria-label="props.labels.resume"
      :aria-busy="props.state.isOperating ? 'true' : undefined"
      :loading="props.state.isOperating"
      :disabled="props.state.isActionDisabled"
      @click="emit('resume')"
    >
      <AppIcon v-if="!props.state.isOperating" name="play" :size="14" />
    </NButton>
    <NButton
      v-if="props.permissions.canConfirmFiles"
      quaternary
      circle
      size="tiny"
      type="primary"
      class="icon-action"
      :title="props.labels.confirmFiles"
      :aria-label="props.labels.confirmFiles"
      :aria-busy="props.state.isOperating ? 'true' : undefined"
      :loading="props.state.isOperating"
      :disabled="props.state.isActionDisabled"
      @click="emit('confirmFiles')"
    >
      <AppIcon name="confirm" :size="14" />
    </NButton>
    <NButton
      v-if="props.permissions.canRedownload"
      quaternary
      circle
      size="tiny"
      class="icon-action"
      :title="props.labels.redownload"
      :aria-label="props.labels.redownload"
      :disabled="props.state.isActionDisabled"
      @click="showRedownloadConfirm = true"
    >
      <AppIcon name="redownload" :size="14" />
    </NButton>
    <NButton
      v-if="props.permissions.canDelete"
      quaternary
      circle
      size="tiny"
      type="error"
      class="icon-action"
      :title="props.labels.delete"
      :aria-label="props.labels.delete"
      :disabled="props.state.isActionDisabled"
      @click="openDeleteConfirm"
    >
      <AppIcon name="delete" :size="14" />
    </NButton>
    <NButton
      v-if="props.permissions.canPermanentDelete"
      quaternary
      circle
      size="tiny"
      type="error"
      class="icon-action"
      :title="props.labels.permanentDelete"
      :aria-label="props.labels.permanentDelete"
      :aria-busy="props.state.isOperating ? 'true' : undefined"
      :loading="props.state.isOperating"
      :disabled="props.state.isActionDisabled"
      @click="showPermanentDeleteConfirm = true"
    >
      <AppIcon name="permanentDelete" :size="14" />
    </NButton>
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

  <TaskDetailsDialog v-model:show="showDetails" :details="props.details" :close-label="props.labels.close" />

  <TaskRedownloadConfirmDialog
    v-model:show="showRedownloadConfirm"
    :state="props.state"
    :labels="props.labels"
    :confirm-texts="props.confirmTexts"
    @confirm="emit('confirmRedownload')"
  />

  <TaskDeleteConfirmDialog
    v-model:show="showDeleteConfirm"
    :state="props.state"
    :labels="props.labels"
    :confirm-texts="props.confirmTexts"
    @confirm="emit('confirmDelete', $event)"
  />

  <TaskPermanentDeleteConfirmDialog
    v-model:show="showPermanentDeleteConfirm"
    :state="props.state"
    :labels="props.labels"
    :confirm-texts="props.confirmTexts"
    @confirm="emit('confirmPermanentDelete')"
  />
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
  --n-width: 26px;
  --n-height: 26px;
  --n-icon-size: 14px;
  color: var(--app-text-dim);
  opacity: 0.62;
}

.icon-action:hover,
.icon-action:focus-visible {
  color: var(--app-text-strong);
  opacity: 1;
}

.icon-action:disabled {
  opacity: 0.38;
}


.compact-actions {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 8px;
}

.compact-actions :deep(.n-button) {
  width: 100%;
}

@media (max-width: 767px) {
  .compact-actions {
    gap: 10px;
  }

  .compact-actions :deep(.n-button) {
    min-height: var(--app-touch-target-min);
    border-radius: var(--app-radius-sm);
  }

  .compact-actions {
    min-width: 0;
  }
}
</style>
