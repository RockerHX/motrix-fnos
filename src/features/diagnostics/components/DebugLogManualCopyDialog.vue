<script setup lang="ts">
import { NButton, NCard, NInput, NModal } from "naive-ui";
import { nextTick, ref, watch } from "vue";
import { useI18n } from "../../../i18n";

type ManualCopyInputRef = {
  focus?: () => void;
  select?: () => void;
  textareaElRef?: HTMLTextAreaElement | null;
  inputElRef?: HTMLInputElement | null;
  $el?: HTMLElement;
};

const props = defineProps<{
  show: boolean;
  text: string;
}>();

const emit = defineEmits<{
  "update:show": [show: boolean];
  download: [];
}>();

const { t } = useI18n();
const manualCopyRef = ref<ManualCopyInputRef | null>(null);

watch(
  () => props.show,
  (show) => {
    if (show) {
      void nextTick(focusInput);
    }
  },
);

function closeDialog() {
  emit("update:show", false);
}

function focusInput() {
  const input = manualCopyRef.value;
  input?.focus?.();
  input?.select?.();

  const nativeInput =
    input?.textareaElRef ??
    input?.inputElRef ??
    (input?.$el?.querySelector?.("textarea, input") as HTMLTextAreaElement | HTMLInputElement | null | undefined);
  nativeInput?.focus();
  nativeInput?.select();
}
</script>

<template>
  <NModal :show="show" @update:show="emit('update:show', $event)">
    <NCard class="manual-copy-dialog app-dialog" role="dialog" aria-modal="true">
      <template #header>
        <div>
          <p class="app-dialog-eyebrow">{{ t("logs.manualCopy.eyebrow") }}</p>
          <h2>{{ t("logs.manualCopy.title") }}</h2>
        </div>
      </template>
      <template #header-extra>
        <NButton quaternary circle :title="t('common.close')" :aria-label="t('common.close')" @click="closeDialog">×</NButton>
      </template>

      <p class="manual-copy-hint">{{ t("logs.manualCopy.hint") }}</p>
      <NInput
        ref="manualCopyRef"
        class="manual-copy-input"
        type="textarea"
        readonly
        :value="text"
        :input-props="{ readonly: true }"
        :autosize="{ minRows: 12, maxRows: 24 }"
      />
      <div class="manual-copy-actions">
        <NButton secondary @click="emit('download')">{{ t("logs.download") }}</NButton>
        <NButton type="primary" @click="closeDialog">{{ t("common.done") }}</NButton>
      </div>
    </NCard>
  </NModal>
</template>

<style scoped>
.manual-copy-dialog {
  --app-dialog-width: 900px;
}

h2 {
  margin: 0;
}

.manual-copy-hint {
  margin: 0 0 12px;
  color: #b8c4be;
  line-height: 1.6;
}

.manual-copy-input {
  width: 100%;
}

.manual-copy-input :deep(textarea),
.manual-copy-input :deep(.n-input__textarea-el) {
  min-height: min(460px, calc(100vh - 260px));
  font: 12px/1.6 ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, "Liberation Mono", monospace;
}

.manual-copy-actions {
  display: flex;
  justify-content: flex-end;
  gap: 10px;
  margin-top: 14px;
}

@media (max-width: 767px) {
  .manual-copy-input :deep(textarea),
  .manual-copy-input :deep(.n-input__textarea-el) {
    min-height: calc(var(--app-viewport-height) - 360px);
    font-size: 16px;
  }

  .manual-copy-actions {
    flex-direction: column-reverse;
  }

  .manual-copy-actions :deep(.n-button) {
    width: 100%;
  }
}
</style>
