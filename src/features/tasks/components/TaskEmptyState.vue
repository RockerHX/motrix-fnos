<script setup lang="ts">
const emit = defineEmits<{
  create: [];
}>();

function createTask() {
  emit("create");
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
    <h1>暂无任务</h1>
    <p>点击下方按钮或粘贴 HTTP / HTTPS 链接开始您的第一次下载。</p>
    <div class="empty-actions">
      <button type="button" class="primary" @click="createTask">
        <span>＋</span>
        添加任务
      </button>
      <button type="button" class="secondary">
        <span>⚙</span>
        打开设置
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
</style>
