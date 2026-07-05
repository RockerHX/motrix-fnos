<script setup lang="ts">
import { computed } from "vue";
import { NButton, NCard, NDescriptions, NDescriptionsItem, NModal, NTag } from "naive-ui";
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
      return "发现新版本";
    case "up_to_date":
      return "已是最新";
    case "unavailable":
      return "检查失败";
    default:
      return "未检查";
  }
}

function architectureLabel(asset: ReleaseAssetInfo) {
  return asset.architecture === "x86" ? "x86_64" : "ARM / aarch64";
}

function targetArchLabel(arch: string | undefined) {
  if (!arch) return "--";
  if (arch === "x86_64") return "x86_64";
  if (arch === "aarch64" || arch === "arm64") return "ARM / aarch64";
  return arch;
}
</script>

<template>
  <NModal :show="props.show" @update:show="updateShow">
    <NCard class="about-dialog" role="dialog" aria-modal="true">
      <template #header>
        <div>
          <p class="eyebrow">About</p>
          <h2>关于 {{ props.appInfo?.name ?? "Motrix" }}</h2>
        </div>
      </template>
      <template #header-extra>
        <NButton quaternary circle title="关闭" aria-label="关闭" @click="closeDialog">×</NButton>
      </template>

      <div class="about-content">
        <section class="about-hero">
          <div class="app-mark" aria-hidden="true">M</div>
          <div>
            <h3>{{ props.appInfo?.name ?? "Motrix" }}</h3>
            <p>飞牛 fnOS 下载管理应用</p>
            <div class="hero-tags">
              <NTag type="success" round>v{{ currentVersion }}</NTag>
              <NTag round>{{ targetArchLabel(props.appInfo?.targetArch) }}</NTag>
            </div>
          </div>
        </section>

        <NDescriptions label-placement="left" bordered :column="1" size="small">
          <NDescriptionsItem label="维护者">{{ props.appInfo?.maintainer ?? "--" }}</NDescriptionsItem>
          <NDescriptionsItem label="后端状态">{{ props.appInfo?.backendStatus ?? "--" }}</NDescriptionsItem>
          <NDescriptionsItem label="更新方式">手动安装 FPK，或上架后通过 fnOS 应用中心更新</NDescriptionsItem>
          <NDescriptionsItem label="项目地址">
            <a :href="props.appInfo?.repositoryUrl" target="_blank" rel="noreferrer">{{ props.appInfo?.repositoryUrl ?? "--" }}</a>
          </NDescriptionsItem>
          <NDescriptionsItem label="发布页面">
            <a :href="props.appInfo?.releasePageUrl" target="_blank" rel="noreferrer">{{ props.appInfo?.releasePageUrl ?? "--" }}</a>
          </NDescriptionsItem>
        </NDescriptions>

        <section class="about-section">
          <div class="section-heading">
            <div>
              <h3>版本检测</h3>
              <p>应用只检查新版本并提供下载入口，不会自动安装或替换 FPK。</p>
            </div>
            <NButton type="primary" :loading="props.isCheckingUpdate" @click="checkUpdate">检查更新</NButton>
          </div>

          <div class="update-result">
            <NTag :type="updateStatusType" round>{{ statusLabel(props.updateCheck?.status) }}</NTag>
            <p>{{ props.updateCheck?.message ?? "尚未检查更新。" }}</p>
          </div>

          <div v-if="props.updateCheck?.latestVersion" class="version-line">
            <span>当前版本：v{{ props.updateCheck.currentVersion }}</span>
            <span>最新版本：v{{ props.updateCheck.latestVersion }}</span>
          </div>

          <div v-if="releaseAssets.length > 0" class="asset-list">
            <a v-for="asset in releaseAssets" :key="asset.name" :href="asset.downloadUrl" target="_blank" rel="noreferrer">
              <strong>{{ architectureLabel(asset) }}</strong>
              <span>{{ asset.name }}</span>
            </a>
          </div>
        </section>
      </div>
    </NCard>
  </NModal>
</template>

<style scoped>
.about-dialog {
  width: min(760px, calc(100vw - 48px));
  max-height: calc(var(--app-viewport-height) - 48px);
  overflow: auto;
}

.eyebrow,
h2,
h3,
p {
  margin: 0;
}

.eyebrow {
  margin-bottom: 6px;
  color: #66e39a;
  font-size: 12px;
  font-weight: 800;
  letter-spacing: 0.1em;
  text-transform: uppercase;
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
  place-items: center;
  border-radius: 16px;
  color: #102010;
  background: #66e39a;
  font-size: 28px;
  font-weight: 900;
}

.about-hero h3,
.about-section h3 {
  color: #eef4ed;
  font-size: 18px;
}

.about-hero p,
.about-section p,
.version-line {
  color: #aeb9ad;
  font-size: 13px;
  line-height: 1.6;
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
  border-radius: 14px;
  background: rgba(255, 255, 255, 0.05);
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
  border-radius: 12px;
  color: #dfe8dc;
  text-decoration: none;
  overflow-wrap: anywhere;
}

.asset-list a:hover {
  border-color: rgba(142, 240, 138, 0.5);
}

a {
  color: #8ef08a;
  overflow-wrap: anywhere;
}
</style>
