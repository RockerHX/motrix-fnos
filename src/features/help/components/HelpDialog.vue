<script setup lang="ts">
import { ref, watch } from "vue";
import { NButton, NCollapse, NCollapseItem, NTag } from "naive-ui";
import AppDialog from "../../../components/ui/AppDialog.vue";
import { useI18n } from "../../../i18n";

type HelpTopic = "authorized-dirs" | "download-settings" | "autostart" | "trash" | "extensions" | "diagnostics" | "json-rpc";

const props = defineProps<{
  show: boolean;
}>();

const emit = defineEmits<{
  "update:show": [show: boolean];
  openRpcGuide: [];
}>();

const { t } = useI18n();
const expandedTopic = ref<HelpTopic | null>("authorized-dirs");

watch(
  () => props.show,
  (show) => {
    if (show) {
      expandedTopic.value = "authorized-dirs";
    }
  },
);

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
    fixed-body
    content-class="help-dialog-content"
    @update:show="updateShow"
  >
    <NCollapse
      v-model:expanded-names="expandedTopic"
      class="help-pane-scroll"
      accordion
      arrow-placement="right"
      :aria-label="t('help.accordion.label')"
    >
      <NCollapseItem name="authorized-dirs" :title="t('help.authorizedDirs.title')">
        <template #header-extra>
          <NTag size="small" type="success" round>{{ t("common.enabled") }}</NTag>
        </template>
        <p class="help-topic-body">{{ t("help.authorizedDirs.body") }}</p>
      </NCollapseItem>

      <NCollapseItem name="download-settings" :title="t('help.downloadSettings.title')">
        <template #header-extra>
          <NTag size="small" type="success" round>{{ t("common.enabled") }}</NTag>
        </template>
        <p class="help-topic-body">{{ t("help.downloadSettings.body") }}</p>
      </NCollapseItem>

      <NCollapseItem name="autostart" :title="t('help.autostart.title')">
        <template #header-extra>
          <NTag size="small" type="warning" round>{{ t("common.pending") }}</NTag>
        </template>
        <p class="help-topic-body">{{ t("help.autostart.body") }}</p>
      </NCollapseItem>

      <NCollapseItem name="trash" :title="t('help.trash.title')">
        <template #header-extra>
          <NTag size="small" round>{{ t("help.trash.tag") }}</NTag>
        </template>
        <p class="help-topic-body">{{ t("help.trash.body") }}</p>
      </NCollapseItem>

      <NCollapseItem name="extensions" :title="t('help.extensions.title')">
        <template #header-extra>
          <NTag size="small" type="warning" round>{{ t("common.placeholder") }}</NTag>
        </template>
        <p class="help-topic-body">{{ t("help.extensions.body") }}</p>
      </NCollapseItem>

      <NCollapseItem name="diagnostics" :title="t('help.diagnostics.title')">
        <template #header-extra>
          <NTag size="small" type="info" round>{{ t("common.troubleshooting") }}</NTag>
        </template>
        <p class="help-topic-body">{{ t("help.diagnostics.body") }}</p>
      </NCollapseItem>

      <NCollapseItem name="json-rpc" :title="t('help.jsonRpc.title')">
        <p class="help-topic-body">{{ t("help.jsonRpc.body") }}</p>
        <div class="help-topic-actions">
          <NButton secondary size="small" @click="emit('openRpcGuide')">
            {{ t("help.jsonRpc.openGuide") }}
          </NButton>
        </div>
      </NCollapseItem>
    </NCollapse>
  </AppDialog>
</template>

<style scoped src="./HelpDialog.css"></style>
