<script setup lang="ts">
import { computed } from "vue";
import type { DownloadTaskStatus } from "../../../types/tasks";

const props = defineProps<{
  status: DownloadTaskStatus;
}>();

const label = computed(() => {
  const labels: Record<DownloadTaskStatus, string> = {
    pending: "排队",
    active: "下载中",
    paused: "暂停",
    complete: "已完成",
    error: "错误",
    removed: "已删除",
  };

  return labels[props.status];
});

const badgeType = computed(() => {
  if (props.status === "active") {
    return "success";
  }
  if (props.status === "error") {
    return "error";
  }
  if (props.status === "complete") {
    return "info";
  }
  if (props.status === "paused") {
    return "warning";
  }
  return "default";
});
</script>

<template>
  <n-tag :type="badgeType" size="small" round>{{ label }}</n-tag>
</template>
