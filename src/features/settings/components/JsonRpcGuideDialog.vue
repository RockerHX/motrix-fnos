<script setup lang="ts">
import AppDialog from "../../../components/ui/AppDialog.vue";
import { useI18n } from "../../../i18n";
import JsonRpcGuide from "./JsonRpcGuide.vue";

const props = defineProps<{
  show: boolean;
}>();

const emit = defineEmits<{
  "update:show": [show: boolean];
  openSettings: [];
}>();

const { t } = useI18n();

function openSettings() {
  emit("update:show", false);
  emit("openSettings");
}
</script>

<template>
  <AppDialog
    :show="props.show"
    :eyebrow="t('rpcGuide.eyebrow')"
    :title="t('rpcGuide.dialogTitle')"
    width="760px"
    @update:show="emit('update:show', $event)"
  >
    <JsonRpcGuide @open-settings="openSettings" />
  </AppDialog>
</template>
