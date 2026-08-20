<script setup lang="ts">
import { computed, watch, ref } from "vue";
import AppIcon from "../../../components/AppIcon.vue";
import { NButton, NSpace } from "naive-ui";
import TaskDetailsDialog from "./TaskDetailsDialog.vue";
import TaskRedownloadConfirmDialog from "./TaskRedownloadConfirmDialog.vue";
import TaskDeleteConfirmDialog from "./TaskDeleteConfirmDialog.vue";
import TaskPermanentDeleteConfirmDialog from "./TaskPermanentDeleteConfirmDialog.vue";
import TaskRestoreConfirmDialog from "./TaskRestoreConfirmDialog.vue";
import type { DownloadTask } from "../../../types/tasks";
import type {
  TaskActionConfirmTexts,
  TaskActionDetails,
  TaskActionLabels,
  TaskActionPermissions,
  TaskActionState,
  TaskFileActionView,
} from "./taskActionViewModel";

const props = withDefaults(
  defineProps<{
    compact?: boolean;
    variant?: "text" | "icon-pill";
    task: DownloadTask;
    state: TaskActionState;
    permissions: TaskActionPermissions;
    labels: TaskActionLabels;
    details: TaskActionDetails;
    confirmTexts: TaskActionConfirmTexts;
    fileActions?: TaskFileActionView;
  }>(),
  {
    compact: false,
    variant: "text",
    fileActions: () => ({ hostSupported: false, loading: false, context: null }),
  },
);

const emit = defineEmits<{
  pause: [];
  resume: [];
  confirmFiles: [];
  confirmRedownload: [useProxy: boolean];
  confirmDelete: [deleteFiles: boolean];
  restore: [useProxy: boolean];
  updateProxy: [enabled: boolean];
  confirmPermanentDelete: [];
  detailsOpened: [];
  openFileManager: [];
  openFile: [];
  showFileDetails: [];
}>();

const showDeleteConfirm = ref(false);
const showPermanentDeleteConfirm = ref(false);
const showRedownloadConfirm = ref(false);
const showRestoreConfirm = ref(false);
const showDetails = ref(false);
const redownloadUseProxy = ref(false);
const restoreUseProxy = ref(false);
const canOpenFileManager = computed(() => props.fileActions?.hostSupported && props.task.status === "complete");

function openDetails() {
  showDetails.value = true;
  emit("detailsOpened");
}

watch(
  () => props.state.isRuntimeExiting,
  (isRuntimeExiting) => {
    if (!isRuntimeExiting) {
      return;
    }

    showDeleteConfirm.value = false;
    showPermanentDeleteConfirm.value = false;
    showRedownloadConfirm.value = false;
    showRestoreConfirm.value = false;
    showDetails.value = false;
  },
);

watch(
  () => [props.permissions.canRedownload, props.permissions.canRestore] as const,
  ([canRedownload, canRestore]) => {
    if (!canRedownload) showRedownloadConfirm.value = false;
    if (!canRestore) showRestoreConfirm.value = false;
  },
);

function openDeleteConfirm() {
  showDeleteConfirm.value = true;
}

function openRedownloadConfirm() {
  redownloadUseProxy.value = props.task.useProxy;
  showRedownloadConfirm.value = true;
}

function openRestoreConfirm() {
  restoreUseProxy.value = props.task.useProxy;
  showRestoreConfirm.value = true;
}
</script>

<template>
  <div v-if="props.variant === 'icon-pill'" class="icon-pill-actions" role="toolbar" :aria-label="props.labels.details">
    <NButton
      quaternary
      circle
      size="tiny"
      class="task-action-button icon-action"
      :title="props.labels.details"
      :aria-label="props.labels.details"
      :disabled="props.state.isActionDisabled"
      @click="openDetails"
    >
      <AppIcon name="info" :size="14" />
    </NButton>
    <NButton
      v-if="canOpenFileManager"
      quaternary
      circle
      size="tiny"
      class="task-action-button icon-action"
      :title="props.labels.openFileManager"
      :aria-label="props.labels.openFileManager"
      :loading="props.fileActions?.loading"
      :disabled="props.state.isActionDisabled"
      @click="emit('openFileManager')"
    >
      <AppIcon v-if="!props.fileActions?.loading" name="folder" :size="14" />
    </NButton>
    <NButton
      v-if="props.permissions.canPause"
      quaternary
      circle
      size="tiny"
      class="task-action-button icon-action"
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
      class="task-action-button icon-action"
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
      class="task-action-button icon-action"
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
      class="task-action-button icon-action"
      :title="props.labels.redownload"
      :aria-label="props.labels.redownload"
      :disabled="props.state.isActionDisabled"
      @click="openRedownloadConfirm"
    >
      <AppIcon name="redownload" :size="14" />
    </NButton>
    <NButton
      v-if="props.permissions.canDelete"
      quaternary
      circle
      size="tiny"
      type="error"
      class="task-action-button icon-action"
      :title="props.labels.delete"
      :aria-label="props.labels.delete"
      :disabled="props.state.isActionDisabled"
      @click="openDeleteConfirm"
    >
      <AppIcon name="delete" :size="14" />
    </NButton>
    <NButton
      v-if="props.permissions.canRestore"
      quaternary
      circle
      size="tiny"
      class="task-action-button icon-action"
      :title="props.labels.restore"
      :aria-label="props.labels.restore"
      :aria-busy="props.state.isOperating ? 'true' : undefined"
      :loading="props.state.isOperating"
      :disabled="props.state.isActionDisabled"
      @click="openRestoreConfirm"
    >
      <AppIcon v-if="!props.state.isOperating" name="restore" :size="14" />
    </NButton>
    <NButton
      v-if="props.permissions.canPermanentDelete"
      quaternary
      circle
      size="tiny"
      type="error"
      class="task-action-button icon-action"
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
      class="task-action-button"
      :title="props.labels.details"
      :aria-label="props.labels.details"
      :disabled="props.state.isActionDisabled"
      @click="openDetails"
    >
      {{ props.labels.details }}
    </NButton>
    <NButton
      v-if="canOpenFileManager"
      size="small"
      secondary
      class="task-action-button"
      :title="props.labels.openFileManager"
      :aria-label="props.labels.openFileManager"
      :loading="props.fileActions?.loading"
      :disabled="props.state.isActionDisabled"
      @click="emit('openFileManager')"
    >
      {{ props.labels.openFileManager }}
    </NButton>
    <NButton
      v-if="props.permissions.canPause"
      size="small"
      secondary
      class="task-action-button"
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
      class="task-action-button"
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
      class="task-action-button"
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
      class="task-action-button"
      :title="props.labels.redownload"
      :aria-label="props.labels.redownload"
      :disabled="props.state.isActionDisabled"
      @click="openRedownloadConfirm"
    >
      {{ props.labels.redownload }}
    </NButton>
    <NButton
      v-if="props.permissions.canDelete"
      size="small"
      secondary
      type="error"
      class="task-action-button"
      :title="props.labels.delete"
      :aria-label="props.labels.delete"
      :disabled="props.state.isActionDisabled"
      @click="openDeleteConfirm"
    >
      {{ props.labels.delete }}
    </NButton>
    <NButton
      v-if="props.permissions.canRestore"
      size="small"
      secondary
      class="task-action-button"
      :title="props.labels.restore"
      :aria-label="props.labels.restore"
      :loading="props.state.isOperating"
      :disabled="props.state.isActionDisabled"
      @click="openRestoreConfirm"
    >
      {{ props.labels.restore }}
    </NButton>
    <NButton
      v-if="props.permissions.canPermanentDelete"
      size="small"
      secondary
      type="error"
      class="task-action-button"
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
    <NButton class="task-action-button" size="small" secondary :disabled="props.state.isActionDisabled" @click="openDetails">
      {{ props.labels.details }}
    </NButton>
    <NButton
      v-if="canOpenFileManager"
      size="small"
      secondary
      class="task-action-button"
      :title="props.labels.openFileManager"
      :aria-label="props.labels.openFileManager"
      :loading="props.fileActions?.loading"
      :disabled="props.state.isActionDisabled"
      @click="emit('openFileManager')"
    >
      {{ props.labels.openFileManager }}
    </NButton>
    <NButton
      v-if="props.permissions.canPause"
      size="small"
      secondary
      class="task-action-button"
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
      class="task-action-button"
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
      class="task-action-button"
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
      class="task-action-button"
      :disabled="props.state.isActionDisabled"
      @click="openRedownloadConfirm"
    >
      {{ props.labels.redownload }}
    </NButton>
    <NButton
      v-if="props.permissions.canDelete"
      size="small"
      secondary
      type="error"
      class="task-action-button"
      :disabled="props.state.isActionDisabled"
      @click="openDeleteConfirm"
    >
      {{ props.labels.delete }}
    </NButton>
    <NButton
      v-if="props.permissions.canRestore"
      size="small"
      secondary
      class="task-action-button"
      :loading="props.state.isOperating"
      :disabled="props.state.isActionDisabled"
      @click="openRestoreConfirm"
    >
      {{ props.labels.restore }}
    </NButton>
    <NButton
      v-if="props.permissions.canPermanentDelete"
      size="small"
      secondary
      type="error"
      class="task-action-button"
      :loading="props.state.isOperating"
      :disabled="props.state.isActionDisabled"
      @click="showPermanentDeleteConfirm = true"
    >
      {{ props.labels.permanentDelete }}
    </NButton>
  </NSpace>

  <TaskDetailsDialog
    v-model:show="showDetails"
    :task="props.task"
    :details="props.details"
    :close-label="props.labels.close"
    :is-operating="props.state.isOperating"
    :is-action-disabled="props.state.isActionDisabled"
    :file-actions="props.fileActions"
    :labels="props.labels"
    @update-proxy="emit('updateProxy', $event)"
    @open-file="emit('openFile')"
    @show-file-details="emit('showFileDetails')"
  />

  <TaskRedownloadConfirmDialog
    v-model:show="showRedownloadConfirm"
    v-model:use-proxy="redownloadUseProxy"
    :state="props.state"
    :labels="props.labels"
    :confirm-texts="props.confirmTexts"
    @confirm="emit('confirmRedownload', $event)"
  />

  <TaskRestoreConfirmDialog
    v-model:show="showRestoreConfirm"
    v-model:use-proxy="restoreUseProxy"
    :state="props.state"
    :labels="props.labels"
    :confirm-texts="props.confirmTexts"
    @confirm="emit('restore', $event)"
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

<style scoped src="./TaskActions.css"></style>
