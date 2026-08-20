<script setup lang="ts">
import { NSwitch } from "naive-ui";
import { useI18n } from "../../../i18n";

const props = withDefaults(
  defineProps<{
    value: boolean;
    disabled?: boolean;
    loading?: boolean;
  }>(),
  {
    disabled: false,
    loading: false,
  },
);

const emit = defineEmits<{
  "update:value": [enabled: boolean];
}>();

const { t } = useI18n();
</script>

<template>
  <div class="task-proxy-toggle" data-test="task-proxy-toggle">
    <div class="task-proxy-toggle-copy">
      <strong>{{ t("task.proxy.label") }}</strong>
      <span>{{ t(props.value ? "task.proxy.on" : "task.proxy.off") }}</span>
    </div>
    <NSwitch
      :value="props.value"
      :disabled="props.disabled"
      :loading="props.loading"
      :aria-label="t('task.proxy.label')"
      @update:value="emit('update:value', $event)"
    />
  </div>
</template>

<style scoped src="./TaskProxyToggle.css"></style>
