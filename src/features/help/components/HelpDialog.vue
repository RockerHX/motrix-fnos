<script setup lang="ts">
import { NButton, NCard, NModal, NTag } from "naive-ui";
import { useI18n } from "../../../i18n";

const props = defineProps<{
  show: boolean;
}>();

const emit = defineEmits<{
  "update:show": [show: boolean];
}>();

const { t } = useI18n();

function updateShow(show: boolean) {
  emit("update:show", show);
}

function closeDialog() {
  updateShow(false);
}
</script>

<template>
  <NModal :show="props.show" @update:show="updateShow">
    <NCard class="help-dialog" role="dialog" aria-modal="true">
      <template #header>
        <div>
          <p class="eyebrow">{{ t("help.eyebrow") }}</p>
          <h2>{{ t("help.title") }}</h2>
        </div>
      </template>
      <template #header-extra>
        <NButton quaternary circle :title="t('common.close')" :aria-label="t('common.close')" @click="closeDialog">×</NButton>
      </template>

      <div class="help-sections">
        <section>
          <div class="section-title">
            <h3>{{ t("help.authorizedDirs.title") }}</h3>
            <NTag size="small" type="success" round>{{ t("common.enabled") }}</NTag>
          </div>
          <p>{{ t("help.authorizedDirs.body") }}</p>
        </section>

        <section>
          <div class="section-title">
            <h3>{{ t("help.downloadSettings.title") }}</h3>
            <NTag size="small" type="success" round>{{ t("common.enabled") }}</NTag>
          </div>
          <p>{{ t("help.downloadSettings.body") }}</p>
        </section>

        <section>
          <div class="section-title">
            <h3>{{ t("help.autostart.title") }}</h3>
            <NTag size="small" type="warning" round>{{ t("common.pending") }}</NTag>
          </div>
          <p>{{ t("help.autostart.body") }}</p>
        </section>

        <section>
          <div class="section-title">
            <h3>{{ t("help.trash.title") }}</h3>
            <NTag size="small" round>{{ t("help.trash.tag") }}</NTag>
          </div>
          <p>{{ t("help.trash.body") }}</p>
        </section>

        <section>
          <div class="section-title">
            <h3>{{ t("help.extensions.title") }}</h3>
            <NTag size="small" type="warning" round>{{ t("common.placeholder") }}</NTag>
          </div>
          <p>{{ t("help.extensions.body") }}</p>
        </section>

        <section>
          <div class="section-title">
            <h3>{{ t("help.diagnostics.title") }}</h3>
            <NTag size="small" type="info" round>{{ t("common.troubleshooting") }}</NTag>
          </div>
          <p>{{ t("help.diagnostics.body") }}</p>
        </section>
      </div>
    </NCard>
  </NModal>
</template>

<style scoped>
.help-dialog {
  width: min(760px, calc(100vw - 48px));
  max-height: calc(var(--app-viewport-height) - 48px);
  overflow: auto;
}

.eyebrow {
  margin: 0 0 6px;
  color: #66e39a;
  font-size: 12px;
  font-weight: 800;
  letter-spacing: 0.1em;
  text-transform: uppercase;
}

h2,
h3,
p {
  margin: 0;
}

.help-sections {
  display: grid;
  gap: 14px;
}

.help-sections section {
  padding: 16px;
  border-radius: 12px;
  background: rgba(255, 255, 255, 0.05);
}

.section-title {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  margin-bottom: 10px;
}

.section-title h3 {
  min-width: 0;
  color: #eef4ed;
  font-size: 16px;
  font-weight: 700;
  overflow-wrap: anywhere;
}

.help-sections p {
  color: #b7bfb4;
  font-size: 14px;
  line-height: 1.7;
  overflow-wrap: anywhere;
}

@media (max-width: 767px) {
  .help-dialog {
    width: calc(100vw - 16px);
    max-height: calc(var(--app-viewport-height) - 16px);
    border-radius: 18px;
  }

  .help-sections {
    gap: 12px;
  }

  .help-sections section {
    padding: 14px;
  }

  .section-title {
    align-items: flex-start;
    flex-wrap: wrap;
  }

  .section-title h3 {
    font-size: 15px;
  }
}
</style>
