<script setup lang="ts">
import { NButton, NEmpty } from "naive-ui";
import AppIcon from "../../../components/AppIcon.vue";
import { useI18n } from "../../../i18n";

const { t } = useI18n();

const props = withDefaults(
  defineProps<{
    title?: string;
    description?: string;
    showCreateAction?: boolean;
    disableCreateAction?: boolean;
    showSettingsAction?: boolean;
  }>(),
  {
    title: "",
    description: "",
    showCreateAction: true,
    disableCreateAction: false,
    showSettingsAction: true,
  },
);

const emit = defineEmits<{
  create: [];
  openSettings: [];
}>();

function createTask() {
  emit("create");
}

function openSettings() {
  emit("openSettings");
}
</script>

<template>
  <section class="empty-guide">
    <NEmpty class="empty-state" size="large" :description="props.title || t('empty.default.title')">
      <template #extra>
        <p class="empty-description">{{ props.description || t("empty.default.description") }}</p>
        <div v-if="props.showCreateAction || props.showSettingsAction" class="empty-actions">
          <NButton
            v-if="props.showCreateAction"
            class="empty-action-button"
            type="primary"
            :title="t('empty.create')"
            :aria-label="t('empty.create')"
            :disabled="props.disableCreateAction"
            @click="createTask"
          >
            <AppIcon name="plus" :size="14" />
            {{ t("empty.create") }}
          </NButton>
          <NButton
            v-if="props.showSettingsAction"
            class="empty-action-button"
            secondary
            :title="t('empty.openSettings')"
            :aria-label="t('empty.openSettings')"
            @click="openSettings"
          >
            <AppIcon name="settings" :size="14" />
            {{ t("empty.openSettings") }}
          </NButton>
        </div>
      </template>
    </NEmpty>
  </section>
</template>

<style scoped src="./TaskEmptyState.css"></style>
