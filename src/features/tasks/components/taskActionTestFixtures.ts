import type { TaskActionConfirmTexts, TaskActionLabels, TaskActionState } from "./taskActionViewModel";

export const defaultState: TaskActionState = {
  isOperating: false,
  isActionDisabled: false,
  isRuntimeExiting: false,
};

export const defaultLabels: TaskActionLabels = {
  details: "详情",
  pause: "暂停",
  resume: "继续",
  confirmFiles: "确认文件",
  redownload: "重新下载",
  delete: "删除",
  permanentDelete: "永久删除",
  cancel: "取消",
  close: "关闭",
};

export const defaultConfirmTexts: TaskActionConfirmTexts = {
  redownloadTitle: "重新下载任务",
  redownloadConfirmText: "确认重新下载",
  deleteTitle: "删除任务",
  deleteConfirmText: "确认删除",
  deleteFilesLabel: "同时删除本地文件",
  permanentDeleteTitle: "永久删除任务记录",
  permanentDeleteConfirmText: "确认永久删除",
};
