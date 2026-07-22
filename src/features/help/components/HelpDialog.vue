<script setup lang="ts">
import { NButton, NTag } from "naive-ui";
import AppDialog from "../../../components/ui/AppDialog.vue";
import AppSectionCard from "../../../components/ui/AppSectionCard.vue";
import { useI18n } from "../../../i18n";

const props = defineProps<{
  show: boolean;
}>();

const emit = defineEmits<{
  "update:show": [show: boolean];
  openRpcGuide: [];
}>();

const { t } = useI18n();

function updateShow(show: boolean) {
  emit("update:show", show);
}

</script>

<template>
  <AppDialog
    :show="props.show"
    :eyebrow="t('help.eyebrow')"
    :title="t('help.title')"
    width="760px"
    @update:show="updateShow"
  >
    <div class="help-sections">
      <AppSectionCard :title="t('help.authorizedDirs.title')" :description="t('help.authorizedDirs.body')">
        <template #meta>
          <NTag size="small" type="success" round>{{ t("common.enabled") }}</NTag>
        </template>
      </AppSectionCard>

      <AppSectionCard :title="t('help.downloadSettings.title')" :description="t('help.downloadSettings.body')">
        <template #meta>
          <NTag size="small" type="success" round>{{ t("common.enabled") }}</NTag>
        </template>
      </AppSectionCard>

      <AppSectionCard :title="t('help.autostart.title')" :description="t('help.autostart.body')">
        <template #meta>
          <NTag size="small" type="warning" round>{{ t("common.pending") }}</NTag>
        </template>
      </AppSectionCard>

      <AppSectionCard :title="t('help.trash.title')" :description="t('help.trash.body')">
        <template #meta>
          <NTag size="small" round>{{ t("help.trash.tag") }}</NTag>
        </template>
      </AppSectionCard>

      <AppSectionCard :title="t('help.extensions.title')" :description="t('help.extensions.body')">
        <template #meta>
          <NTag size="small" type="warning" round>{{ t("common.placeholder") }}</NTag>
        </template>
      </AppSectionCard>

      <AppSectionCard :title="t('help.diagnostics.title')" :description="t('help.diagnostics.body')">
        <template #meta>
          <NTag size="small" type="info" round>{{ t("common.troubleshooting") }}</NTag>
        </template>
      </AppSectionCard>

      <AppSectionCard :title="t('help.jsonRpc.title')" :description="t('help.jsonRpc.body')">
        <template #actions>
          <NButton secondary size="small" @click="emit('openRpcGuide')">
            {{ t("help.jsonRpc.openGuide") }}
          </NButton>
        </template>
      </AppSectionCard>
    </div>
  </AppDialog>
</template>

<style scoped src="./HelpDialog.css"></style>
