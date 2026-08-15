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
  restore: "恢复",
  permanentDelete: "永久删除",
  cancel: "取消",
  close: "关闭",
  openFileManager: "在文件管理器中打开",
  openFile: "打开文件",
  fileDetails: "文件详情",
  hostOnly: "文件操作仅支持 fnOS 宿主环境。",
  technicalInfo: "技术信息",
  copyPath: "复制",
  copied: "已复制",
  copyFailed: "复制失败",
};

export const defaultConfirmTexts: TaskActionConfirmTexts = {
  redownloadTitle: "重新下载任务",
  redownloadConfirmText: "确认重新下载",
  restoreTitle: "恢复下载任务",
  restoreConfirmText: "确认恢复",
  deleteTitle: "删除任务",
  deleteConfirmText: "确认删除",
  deleteFilesLabel: "同时删除本地文件",
  permanentDeleteTitle: "永久删除任务记录",
  permanentDeleteConfirmText: "确认永久删除",
};
