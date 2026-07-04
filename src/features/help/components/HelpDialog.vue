<script setup lang="ts">
import { NButton, NCard, NModal, NTag } from "naive-ui";

const props = defineProps<{
  show: boolean;
}>();

const emit = defineEmits<{
  "update:show": [show: boolean];
}>();

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
          <p class="eyebrow">Help</p>
          <h2>Motrix 使用帮助</h2>
        </div>
      </template>
      <template #header-extra>
        <NButton quaternary circle @click="closeDialog">×</NButton>
      </template>

      <div class="help-sections">
        <section>
          <div class="section-title">
            <h3>授权目录与默认下载目录</h3>
            <NTag size="small" type="success" round>已生效</NTag>
          </div>
          <p>
            默认下载目录来自飞牛应用设置中的已授权文件夹。请先在 fnOS 应用设置里添加读写目录，Motrix
            会优先使用授权目录中的 data 目录；新建任务默认使用设置页保存的目录。
          </p>
        </section>

        <section>
          <div class="section-title">
            <h3>下载设置</h3>
            <NTag size="small" type="success" round>已生效</NTag>
          </div>
          <p>
            最大并发下载数、下载限速和上传限速会保存到后端配置，并尽量即时同步到 Aria2；如果 Aria2 RPC
            暂未就绪，则会在下次服务启动后按保存配置生效。
          </p>
        </section>

        <section>
          <div class="section-title">
            <h3>开机自启与下载通知</h3>
            <NTag size="small" type="warning" round>待支持</NTag>
          </div>
          <p>
            当前版本不会在应用内修改 fnOS 系统开机自启状态，也不会申请浏览器通知权限或调用 fnOS 系统通知能力。
            后续实现前需要先确认飞牛官方接口或完成实机验证。
          </p>
        </section>

        <section>
          <div class="section-title">
            <h3>Trash 与永久删除</h3>
            <NTag size="small" round>记录管理</NTag>
          </div>
          <p>
            Trash 页面只展示已删除的 Motrix 任务记录。永久删除只清理 Motrix 记录和关联历史 / 错误信息，
            不会删除用户下载文件。
          </p>
        </section>

        <section>
          <div class="section-title">
            <h3>Extensions</h3>
            <NTag size="small" type="warning" round>占位页</NTag>
          </div>
          <p>
            当前 FPK Web 版暂未提供插件运行时，不会加载第三方脚本，也不会联网拉取插件。
          </p>
        </section>

        <section>
          <div class="section-title">
            <h3>日志与诊断</h3>
            <NTag size="small" type="info" round>排障入口</NTag>
          </div>
          <p>
            如遇到下载、目录授权或 Aria2 连接问题，可点击右上角诊断按钮查看后端状态、Aria2 状态和调试日志。
          </p>
        </section>
      </div>
    </NCard>
  </NModal>
</template>

<style scoped>
.help-dialog {
  width: min(760px, calc(100vw - 48px));
  max-height: calc(100vh - 48px);
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
  color: #eef4ed;
  font-size: 16px;
  font-weight: 700;
}

.help-sections p {
  color: #b7bfb4;
  font-size: 14px;
  line-height: 1.7;
}
</style>
