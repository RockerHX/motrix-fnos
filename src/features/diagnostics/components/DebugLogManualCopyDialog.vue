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

<style scoped src="./DebugLogManualCopyDialog.css"></style>
