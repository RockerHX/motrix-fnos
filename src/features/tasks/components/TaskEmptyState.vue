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

<style scoped>
.empty-guide {
  height: 100%;
  display: grid;
  place-items: center;
  padding: 24px;
  text-align: center;
}

.empty-state {
  width: min(100%, 420px);
}

.empty-state :deep(.n-empty__description) {
  color: var(--app-text-strong);
  font-size: 20px;
  font-weight: 400;
  line-height: 1.35;
}

.empty-description {
  max-width: 360px;
  margin: 0 auto 22px;
  color: var(--app-text-muted);
  font-size: 14px;
  line-height: 1.5;
}

.empty-actions {
  display: flex;
  justify-content: center;
  gap: 10px;
}

.empty-action-button {
  min-width: 104px;
  --n-border-radius: 7px;
}

@media (min-width: 768px) {
  .empty-actions {
    display: none;
  }
}

@media (max-width: 767px) {
  .empty-guide {
    padding: 24px var(--app-mobile-page-gutter);
  }

  .empty-state :deep(.n-empty__description) {
    font-size: 20px;
  }

  .empty-description {
    max-width: 100%;
    margin-bottom: 20px;
    font-size: 13px;
    line-height: 1.6;
  }

  .empty-actions {
    width: 100%;
    flex-direction: column;
    gap: 12px;
  }

  .empty-action-button {
    width: 100%;
    min-width: 0;
    min-height: var(--app-touch-target-min);
    gap: 8px;
    --n-border-radius: var(--app-radius-sm);
    --n-padding: 0 16px;
    font-size: 15px;
  }
}
</style>
