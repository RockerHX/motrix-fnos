<script setup lang="ts">
import { computed } from "vue";
import { NButton, NDescriptions, NDescriptionsItem, NTag } from "naive-ui";
import AppDialog from "../../../components/ui/AppDialog.vue";
import AppSectionCard from "../../../components/ui/AppSectionCard.vue";
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
  openRpcGuide: [];
}>();

const { t } = useI18n();
const appName = computed(() => props.appInfo?.name ?? "Motrix");
const currentVersion = computed(() => props.appInfo?.version ?? "--");
const updateStatusType = computed(() => statusTagType(props.updateCheck?.status));
const releaseAssets = computed(() => props.updateCheck?.assets ?? []);

function updateShow(show: boolean) {
  emit("update:show", show);
}

function checkUpdate() {
  emit("checkUpdate");
}

function openRpcGuide() {
  emit("openRpcGuide");
  emit("update:show", false);
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
  <AppDialog
    :show="props.show"
    :eyebrow="t('about.eyebrow')"
    :title="t('about.title', { name: appName })"
    width="760px"
    @update:show="updateShow"
  >
    <div class="about-content">
        <section class="about-hero">
          <div class="app-mark" aria-hidden="true">M</div>
          <div>
            <h3>{{ appName }}</h3>
            <p>{{ t("about.subtitle") }}</p>
            <div class="hero-tags">
              <NTag type="primary" round>v{{ currentVersion }}</NTag>
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

        <AppSectionCard
          class="about-update-card"
          :title="t('about.update.title')"
          :description="t('about.update.description')"
        >
          <template #actions>
            <NButton
              type="primary"
              :loading="props.isCheckingUpdate"
              :title="t('about.update.check')"
              :aria-label="t('about.update.check')"
              @click="checkUpdate"
            >
              {{ t("about.update.check") }}
            </NButton>
          </template>

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
        </AppSectionCard>

        <AppSectionCard
          class="about-rpc-guide-entry"
          :title="t('about.rpcGuide.title')"
          :description="t('about.rpcGuide.description')"
        >
          <template #actions>
            <NButton secondary size="small" @click="openRpcGuide">
              {{ t("about.rpcGuide.openGuide") }}
            </NButton>
          </template>
        </AppSectionCard>

        <AppSectionCard
          :title="t('about.changelog.title')"
          :description="t('about.changelog.description')"
        >
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
        </AppSectionCard>
    </div>
  </AppDialog>
</template>

<style scoped src="./AboutDialog.css"></style>
