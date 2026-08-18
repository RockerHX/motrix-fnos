<script setup lang="ts">
import { computed, useSlots, type CSSProperties } from "vue";
import { NButton, NCard, NModal } from "naive-ui";

const props = withDefaults(
  defineProps<{
    show: boolean;
    title?: string;
    eyebrow?: string;
    width?: string;
    maskClosable?: boolean;
    closeDisabled?: boolean;
    showClose?: boolean;
    cardClass?: string;
    contentClass?: string;
    contentStyle?: string | CSSProperties;
  }>(),
  {
    title: "",
    eyebrow: "",
    width: "720px",
    maskClosable: true,
    closeDisabled: false,
    showClose: true,
    cardClass: "",
    contentClass: "",
  },
);

const emit = defineEmits<{
  "update:show": [show: boolean];
  close: [];
}>();

const slots = useSlots();
const cardClasses = computed(() => ["app-dialog", props.cardClass].filter(Boolean));
const cardStyle = computed(() => ({
  "--app-dialog-width": props.width,
}));
const hasHeader = computed(() => Boolean(slots.header || props.title || props.eyebrow));
const hasHeaderExtra = computed(() => Boolean(slots["header-extra"] || props.showClose));

function updateShow(show: boolean) {
  if (!show && props.closeDisabled) {
    return;
  }
  emit("update:show", show);
}

function closeDialog() {
  if (props.closeDisabled) {
    return;
  }
  emit("update:show", false);
  emit("close");
}
</script>

<template>
  <NModal :show="props.show" :mask-closable="props.maskClosable" @update:show="updateShow">
    <NCard
      :class="cardClasses"
      :style="cardStyle"
      :content-class="props.contentClass"
      :content-style="props.contentStyle"
      role="dialog"
      aria-modal="true"
    >
      <template v-if="hasHeader" #header>
        <slot name="header">
          <div>
            <p v-if="props.eyebrow" class="app-dialog-eyebrow">{{ props.eyebrow }}</p>
            <h2 v-if="props.title" class="app-dialog-title">{{ props.title }}</h2>
          </div>
        </slot>
      </template>
      <template v-if="hasHeaderExtra" #header-extra>
        <div class="app-dialog-header-actions">
          <slot name="header-extra" />
          <NButton
            v-if="props.showClose"
            quaternary
            circle
            :disabled="props.closeDisabled"
            aria-label="关闭"
            title="关闭"
            @click="closeDialog"
          >
            ×
          </NButton>
        </div>
      </template>

      <slot />

      <template v-if="$slots.footer" #footer>
        <slot name="footer" />
      </template>
    </NCard>
  </NModal>
</template>

<style scoped src="./AppDialog.css"></style>
