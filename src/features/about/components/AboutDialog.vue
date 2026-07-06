<script setup lang="ts">
import { computed } from "vue";
import { NButton, NCard, NDescriptions, NDescriptionsItem, NModal, NTag } from "naive-ui";
import { useI18n } from "../../../i18n";
import { recentChangelogEntries } from "../services/changelogService";
import type { AppInfo, AppUpdateCheck, ReleaseAssetInfo, UpdateCheckStatus } from "../../../types/app";

const props = defineProps<{
  show: boolean;
  appInfo: AppInfo | null;
  updateCheck: AppUpdateCheck | null;
  isCheckingUpdate?: boolean;
}>();

const emit = defineEmits<{
  "update:show": [show: boolean];
  checkUpdate: [];
}>();

const { t } = useI18n();
const appName = computed(() => props.appInfo?.name ?? "Motrix");
const currentVersion = computed(() => props.appInfo?.version ?? "--");
const updateStatusType = computed(() => statusTagType(props.updateCheck?.status));
const releaseAssets = computed(() => props.updateCheck?.assets ?? []);

function updateShow(show: boolean) {
  emit("update:show", show);
}

function closeDialog() {
  updateShow(false);
}

function checkUpdate() {
  emit("checkUpdate");
}

function statusTagType(status: UpdateCheckStatus | undefined) {
  switch (status) {
    case "available":
      return "warning";
    case "up_to_date":
      return "success";
    case "unavailable":
      return "error";
    default:
      return "default";
  }
}

function statusLabel(status: UpdateCheckStatus | undefined) {
  switch (status) {
    case "available":
      return t("about.update.status.available");
    case "up_to_date":
      return t("about.update.status.upToDate");
    case "unavailable":
      return t("about.update.status.unavailable");
    default:
      return t("about.update.status.unchecked");
  }
}

function architectureLabel(asset: ReleaseAssetInfo) {
  return asset.architecture === "x86" ? t("about.arch.x86") : t("about.arch.arm");
}

function targetArchLabel(arch: string | undefined) {
  if (!arch) return "--";
  if (arch === "x86_64") return t("about.arch.x86");
  if (arch === "aarch64" || arch === "arm64") return t("about.arch.arm");
  return arch;
}
</script>

<template>
  <NModal :show="props.show" @update:show="updateShow">
    <NCard class="about-dialog app-dialog" role="dialog" aria-modal="true">
      <template #header>
        <div>
          <p class="app-dialog-eyebrow">{{ t("about.eyebrow") }}</p>
          <h2>{{ t("about.title", { name: appName }) }}</h2>
        </div>
      </template>
      <template #header-extra>
        <NButton quaternary circle :title="t('common.close')" :aria-label="t('common.close')" @click="closeDialog">×</NButton>
      </template>

      <div class="about-content">
        <section class="about-hero">
          <div class="app-mark" aria-hidden="true">M</div>
          <div>
            <h3>{{ appName }}</h3>
            <p>{{ t("about.subtitle") }}</p>
            <div class="hero-tags">
              <NTag type="success" round>v{{ currentVersion }}</NTag>
              <NTag round>{{ targetArchLabel(props.appInfo?.targetArch) }}</NTag>
            </div>
          </div>
        </section>

        <NDescriptions label-placement="left" bordered :column="1" size="small">
          <NDescriptionsItem :label="t('about.maintainer')">{{ props.appInfo?.maintainer ?? "--" }}</NDescriptionsItem>
          <NDescriptionsItem :label="t('about.backendStatus')">{{ props.appInfo?.backendStatus ?? "--" }}</NDescriptionsItem>
          <NDescriptionsItem :label="t('about.updateMode')">{{ t("about.updateMode.manual") }}</NDescriptionsItem>
          <NDescriptionsItem :label="t('about.repository')">
            <a :href="props.appInfo?.repositoryUrl" :title="props.appInfo?.repositoryUrl" target="_blank" rel="noreferrer">{{ props.appInfo?.repositoryUrl ?? "--" }}</a>
          </NDescriptionsItem>
          <NDescriptionsItem :label="t('about.releases')">
            <a :href="props.appInfo?.releasePageUrl" :title="props.appInfo?.releasePageUrl" target="_blank" rel="noreferrer">{{ props.appInfo?.releasePageUrl ?? "--" }}</a>
          </NDescriptionsItem>
        </NDescriptions>

        <section class="about-section">
          <div class="section-heading">
            <div>
              <h3>{{ t("about.update.title") }}</h3>
              <p>{{ t("about.update.description") }}</p>
            </div>
            <NButton
              type="primary"
              :loading="props.isCheckingUpdate"
              :title="t('about.update.check')"
              :aria-label="t('about.update.check')"
              @click="checkUpdate"
            >
              {{ t("about.update.check") }}
            </NButton>
          </div>

          <div class="update-result">
            <NTag :type="updateStatusType" round>{{ statusLabel(props.updateCheck?.status) }}</NTag>
            <p>{{ props.updateCheck?.message ?? t("about.update.notChecked") }}</p>
          </div>

          <div v-if="props.updateCheck?.latestVersion" class="version-line">
            <span>{{ t("about.update.currentVersion", { version: props.updateCheck.currentVersion }) }}</span>
            <span>{{ t("about.update.latestVersion", { version: props.updateCheck.latestVersion }) }}</span>
          </div>

          <div v-if="releaseAssets.length > 0" class="asset-list">
            <a v-for="asset in releaseAssets" :key="asset.name" :href="asset.downloadUrl" :title="asset.name" target="_blank" rel="noreferrer">
              <strong>{{ architectureLabel(asset) }}</strong>
              <span>{{ asset.name }}</span>
            </a>
          </div>
        </section>
        <section class="about-section">
          <div class="section-heading">
            <div>
              <h3>{{ t("about.changelog.title") }}</h3>
              <p>{{ t("about.changelog.description") }}</p>
            </div>
          </div>

          <div class="changelog-list">
            <article v-for="entry in recentChangelogEntries" :key="`${entry.version}-${entry.date}`" class="changelog-entry">
              <header>
                <strong>{{ entry.version }}</strong>
                <span v-if="entry.date">{{ entry.date }}</span>
              </header>
              <section v-for="section in entry.sections" :key="section.title" class="changelog-section">
                <h4>{{ section.title }}</h4>
                <ul>
                  <li v-for="item in section.items" :key="item">{{ item }}</li>
                </ul>
              </section>
            </article>
          </div>
        </section>
      </div>
    </NCard>
  </NModal>
</template>

<style scoped>
.about-dialog {
  --app-dialog-width: 760px;
}

h2,
h3,
p {
  margin: 0;
}

.about-content {
  display: grid;
  gap: 18px;
}

.about-hero {
  display: flex;
  align-items: center;
  gap: 16px;
}

.app-mark {
  width: 56px;
  height: 56px;
  display: grid;
  flex: 0 0 auto;
  place-items: center;
  border-radius: var(--app-radius-lg);
  color: #102010;
  background: var(--app-text-accent);
  font-size: 28px;
  font-weight: 900;
}

.about-hero h3,
.about-section h3 {
  color: var(--app-text-strong);
  font-size: 18px;
  overflow-wrap: anywhere;
}

.about-hero p,
.about-section p,
.version-line {
  color: #aeb9ad;
  font-size: 13px;
  line-height: 1.6;
  overflow-wrap: anywhere;
}

.hero-tags {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  margin-top: 10px;
}

.about-section {
  display: grid;
  gap: 14px;
  padding: 16px;
  border-radius: var(--app-radius-md);
  background: var(--app-color-card-overlay);
}

.section-heading {
  display: flex;
  justify-content: space-between;
  gap: 16px;
}

.update-result {
  display: flex;
  align-items: flex-start;
  gap: 10px;
}

.version-line {
  display: flex;
  flex-wrap: wrap;
  gap: 12px;
}

.asset-list {
  display: grid;
  gap: 8px;
}

.asset-list a {
  display: grid;
  gap: 4px;
  padding: 10px 12px;
  border: 1px solid rgba(142, 240, 138, 0.22);
  border-radius: var(--app-radius-sm);
  color: #dfe8dc;
  text-decoration: none;
  overflow-wrap: anywhere;
}

.asset-list a:hover {
  border-color: rgba(142, 240, 138, 0.5);
}

.changelog-list {
  display: grid;
  gap: 12px;
}

.changelog-entry {
  display: grid;
  gap: 10px;
  padding: 12px;
  border-radius: var(--app-radius-sm);
  background: rgba(0, 0, 0, 0.16);
}

.changelog-entry header {
  display: flex;
  justify-content: space-between;
  gap: 12px;
  color: var(--app-text-strong);
}

.changelog-entry header span,
.changelog-section h4 {
  color: #9fae9d;
  font-size: 12px;
}

.changelog-section {
  display: grid;
  gap: 6px;
}

.changelog-section h4 {
  margin: 0;
}

.changelog-section ul {
  margin: 0;
  padding-left: 18px;
  color: #c8d2c5;
  font-size: 13px;
  line-height: 1.7;
  overflow-wrap: anywhere;
}

a {
  color: #8ef08a;
  overflow-wrap: anywhere;
}

@media (max-width: 767px) {
  .about-content {
    gap: 14px;
  }

  .about-hero,
  .section-heading,
  .update-result {
    align-items: flex-start;
    flex-direction: column;
  }

  .about-section {
    padding: 14px;
  }

  .section-heading :deep(.n-button) {
    width: 100%;
  }

  .changelog-entry header {
    flex-direction: column;
    gap: 4px;
  }
}
</style>
