<script setup lang="ts">
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
    <div class="empty-box" aria-hidden="true">
      <div class="box-lid" />
      <div class="box-body">
        <span><AppIcon name="plus" :size="16" /></span>
      </div>
    </div>
    <h1>{{ props.title || t("empty.default.title") }}</h1>
    <p>{{ props.description || t("empty.default.description") }}</p>
    <div v-if="props.showCreateAction || props.showSettingsAction" class="empty-actions">
      <button
        v-if="props.showCreateAction"
        type="button"
        class="primary"
        :title="t('empty.create')"
        :aria-label="t('empty.create')"
        :disabled="props.disableCreateAction"
        @click="createTask"
      >
        <AppIcon name="plus" :size="14" />
        {{ t("empty.create") }}
      </button>
      <button
        v-if="props.showSettingsAction"
        type="button"
        class="secondary"
        :title="t('empty.openSettings')"
        :aria-label="t('empty.openSettings')"
        @click="openSettings"
      >
        <AppIcon name="settings" :size="14" />
        {{ t("empty.openSettings") }}
      </button>
    </div>
  </section>
</template>

<style scoped>
.empty-guide {
  height: 100%;
  display: grid;
  justify-items: center;
  align-content: center;
  padding-bottom: 18px;
  text-align: center;
}

.empty-box {
  position: relative;
  width: 88px;
  height: 88px;
  margin-bottom: 28px;
  color: color-mix(in srgb, var(--app-text-muted) 38%, transparent);
}

.box-lid {
  position: absolute;
  left: 15px;
  top: 5px;
  width: 58px;
  height: 24px;
  border: 3px solid color-mix(in srgb, var(--app-text-muted) 28%, transparent);
  border-bottom: 0;
  transform: skewX(-38deg);
}

.box-body {
  position: absolute;
  left: 8px;
  top: 27px;
  width: 72px;
  height: 62px;
  display: grid;
  place-items: center;
  border: 3px solid color-mix(in srgb, var(--app-text-muted) 28%, transparent);
  border-radius: 0 0 14px 14px;
}

.box-body span {
  width: 22px;
  height: 22px;
  display: grid;
  place-items: center;
  border-radius: 999px;
  color: #101710;
  background: var(--app-text-accent);
  line-height: 1;
}

.empty-guide h1 {
  margin: 0 0 10px;
  color: var(--app-text-strong);
  font-size: 20px;
  font-weight: 400;
}

.empty-guide p {
  max-width: 340px;
  margin: 0 0 22px;
  color: var(--app-text-muted);
  font-size: 14px;
  line-height: 1.5;
}

.empty-actions {
  display: flex;
  justify-content: center;
  gap: 10px;
}

button {
  font: inherit;
}

.primary,
.secondary {
  min-width: 104px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  border-radius: 7px;
  padding: 8px 14px;
  font-size: 14px;
  cursor: pointer;
}

.primary {
  border: 1px solid #68ae5a;
  color: #101710;
  background: #68ae5a;
}

.secondary {
  border: 1px solid #3d423d;
  color: #dbe3d8;
  background: transparent;
}

.primary:disabled,
.secondary:disabled {
  opacity: 0.56;
  cursor: not-allowed;
}

@media (min-width: 768px) {
  .empty-guide {
    align-content: start;
    justify-items: start;
    padding: min(22vh, 180px) var(--app-desktop-content-gutter-x) 24px;
    text-align: left;
  }

  .empty-actions {
    display: none;
  }
}

@media (max-width: 767px) {
  .empty-guide {
    align-content: start;
    gap: 0;
    padding: 24px var(--app-mobile-page-gutter) 20px;
  }

  .empty-box {
    width: 88px;
    height: 88px;
    margin-bottom: 28px;
  }

  .box-lid {
    left: 15px;
    top: 5px;
    width: 58px;
    height: 24px;
    border-width: 3px;
  }

  .box-body {
    left: 8px;
    top: 27px;
    width: 72px;
    height: 62px;
    border-width: 3px;
    border-radius: 0 0 14px 14px;
  }

  .box-body span {
    width: 24px;
    height: 24px;
    font-size: 22px;
  }

  .empty-guide h1 {
    margin-bottom: 8px;
    font-size: 20px;
    line-height: 1.35;
  }

  .empty-guide p {
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

  .primary,
  .secondary {
    width: 100%;
    min-width: 0;
    min-height: var(--app-touch-target-min);
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    border-radius: var(--app-radius-sm);
    padding: 12px 16px;
    font-size: 15px;
  }
}
</style>
