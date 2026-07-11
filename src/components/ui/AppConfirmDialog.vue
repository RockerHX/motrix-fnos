<script setup lang="ts">
import { NButton } from "naive-ui";
import AppDialog from "./AppDialog.vue";
import AppDialogActions from "./AppDialogActions.vue";

const props = withDefaults(
  defineProps<{
    show: boolean;
    title: string;
    confirmText?: string;
    confirmType?: "primary" | "error" | "warning";
    loading?: boolean;
    disabled?: boolean;
    maskClosable?: boolean;
    width?: string;
  }>(),
  {
    confirmText: "",
    confirmType: "primary",
    loading: false,
    disabled: false,
    maskClosable: true,
    width: "420px",
  },
);

const emit = defineEmits<{
  "update:show": [show: boolean];
  confirm: [];
  cancel: [];
}>();

function updateShow(show: boolean) {
  emit("update:show", show);
}

function cancel() {
  if (props.loading || props.disabled) {
    return;
  }
  emit("cancel");
  emit("update:show", false);
}

function confirm() {
  if (props.loading || props.disabled) {
    return;
  }
  emit("confirm");
}
</script>

<template>
  <AppDialog
    :show="props.show"
    :title="props.title"
    :width="props.width"
    :mask-closable="props.maskClosable"
    :close-disabled="props.loading || props.disabled"
    @update:show="updateShow"
  >
    <slot>
      <p v-if="props.confirmText" class="app-confirm-text">{{ props.confirmText }}</p>
    </slot>
    <slot name="extra" />

    <template #footer>
      <AppDialogActions>
        <NButton :disabled="props.loading || props.disabled" @click="cancel">取消</NButton>
        <NButton :type="props.confirmType" :loading="props.loading" :disabled="props.disabled" @click="confirm">
          <slot name="confirm-label">确认</slot>
        </NButton>
      </AppDialogActions>
    </template>
  </AppDialog>
</template>

<style scoped>
.app-confirm-text {
  margin: 0 0 14px;
  color: var(--app-text-secondary);
  line-height: 1.6;
  word-break: break-word;
}
</style>
