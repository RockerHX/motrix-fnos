import type { CreateTaskAdvancedOptions, DownloadTaskStartMode } from "../../../types/tasks";

export const DEFAULT_TASK_CATEGORY = "默认";

export type TaskCreateInputType = "url" | "torrent" | "magnet";

export interface TaskCreateFormState {
  urls: string;
  magnet: string;
  torrentFile: File | null;
  saveDir: string;
  startMode: DownloadTaskStartMode;
  category: string;
  connections: number;
  downloadLimitKb: number;
  proxy: string;
}

export function createTaskCreateFormState(): TaskCreateFormState {
  return {
    urls: "",
    magnet: "",
    torrentFile: null,
    saveDir: "",
    startMode: "now",
    category: DEFAULT_TASK_CATEGORY,
    connections: 16,
    downloadLimitKb: 0,
    proxy: "",
  };
}

export function resetTaskCreateFormState(form: TaskCreateFormState) {
  Object.assign(form, createTaskCreateFormState());
}

export function buildTaskAdvancedOptions(form: TaskCreateFormState): CreateTaskAdvancedOptions {
  return {
    connections: form.connections,
    downloadLimitKb: form.downloadLimitKb,
    proxy: optionalText(form.proxy),
  };
}

export function normalizeTaskCategory(category: string) {
  return optionalText(category) || DEFAULT_TASK_CATEGORY;
}

function optionalText(value: string) {
  const trimmed = value.trim();
  return trimmed ? trimmed : null;
}
