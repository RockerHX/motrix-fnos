<script setup lang="ts">
import { NTag } from "naive-ui";
import AppDialog from "../../../components/ui/AppDialog.vue";
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
  </AppDialog>
</template>

<style scoped>
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
  border-radius: var(--app-radius-sm);
  background: var(--app-color-card-overlay);
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
  color: var(--app-text-strong);
  font-size: 16px;
  font-weight: 700;
  overflow-wrap: anywhere;
}

.help-sections p {
  color: var(--app-text-muted);
  font-size: 14px;
  line-height: 1.7;
  overflow-wrap: anywhere;
}

@media (max-width: 767px) {
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
