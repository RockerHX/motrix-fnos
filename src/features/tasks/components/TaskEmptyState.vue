<script setup lang="ts">
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
        <span>+</span>
      </div>
    </div>
    <h1>{{ props.title || t("empty.default.title") }}</h1>
    <p>{{ props.description || t("empty.default.description") }}</p>
    <div v-if="props.showCreateAction || props.showSettingsAction" class="empty-actions">
      <button
        v-if="props.showCreateAction"
        type="button"
        class="primary"
        :disabled="props.disableCreateAction"
        @click="createTask"
      >
        <span>＋</span>
        {{ t("empty.create") }}
      </button>
      <button v-if="props.showSettingsAction" type="button" class="secondary" @click="openSettings">
        <span>⚙</span>
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
  padding-bottom: 24px;
  text-align: center;
}

.empty-box {
  position: relative;
  width: 120px;
  height: 120px;
  margin-bottom: 44px;
  color: #565e55;
}

.box-lid {
  position: absolute;
  left: 20px;
  top: 6px;
  width: 78px;
  height: 32px;
  border: 4px solid #3d423d;
  border-bottom: 0;
  transform: skewX(-38deg);
}

.box-body {
  position: absolute;
  left: 10px;
  top: 34px;
  width: 100px;
  height: 88px;
  display: grid;
  place-items: center;
  border: 4px solid #3d423d;
  border-radius: 0 0 18px 18px;
}

.box-body span {
  width: 28px;
  height: 28px;
  display: grid;
  place-items: center;
  border-radius: 999px;
  color: #101710;
  background: #68ae5a;
  font-size: 26px;
  font-weight: 700;
  line-height: 1;
}

.empty-guide h1 {
  margin: 0 0 14px;
  color: #f1f2ed;
  font-size: 24px;
  font-weight: 400;
}

.empty-guide p {
  max-width: 360px;
  margin: 0 0 30px;
  color: #b7bfb4;
  font-size: 14px;
  line-height: 1.5;
}

.empty-actions {
  display: flex;
  justify-content: center;
  gap: 14px;
}

button {
  font: inherit;
}

.primary,
.secondary {
  min-width: 118px;
  border-radius: 7px;
  padding: 10px 18px;
  font-size: 16px;
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

@media (max-width: 767px) {
  .empty-guide {
    align-content: start;
    gap: 0;
    padding: 28px var(--app-mobile-page-gutter) 24px;
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
    margin-bottom: 10px;
    font-size: 21px;
  }

  .empty-guide p {
    max-width: 100%;
    margin-bottom: 22px;
    font-size: 13px;
  }

  .empty-actions {
    width: 100%;
    flex-direction: column;
    gap: 10px;
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
    padding: 12px 16px;
  }
}
</style>
