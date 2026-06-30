import { invoke } from "@tauri-apps/api/core";
import type { CreateDownloadTaskRequest, DownloadTask } from "../types/tasks";

export function createDownloadTask(payload: CreateDownloadTaskRequest): Promise<DownloadTask> {
  return invoke<DownloadTask>("create_download_task", { payload });
}

export function listDownloadTasks(): Promise<DownloadTask[]> {
  return invoke<DownloadTask[]>("list_download_tasks");
}
