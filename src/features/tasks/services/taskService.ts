import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import type { CreateDownloadTaskRequest, DownloadTask } from "../../../types/tasks";

export function createDownloadTask(payload: CreateDownloadTaskRequest): Promise<DownloadTask> {
  return invoke<DownloadTask>("create_download_task", { payload });
}

export function listDownloadTasks(): Promise<DownloadTask[]> {
  return invoke<DownloadTask[]>("list_download_tasks");
}

export function pauseDownloadTask(taskId: number): Promise<DownloadTask> {
  return invoke<DownloadTask>("pause_download_task", { taskId });
}

export function resumeDownloadTask(taskId: number): Promise<DownloadTask> {
  return invoke<DownloadTask>("resume_download_task", { taskId });
}

export function deleteDownloadTask(taskId: number, deleteFiles: boolean): Promise<DownloadTask> {
  return invoke<DownloadTask>("delete_download_task", { taskId, deleteFiles });
}

export async function selectDownloadDirectory(): Promise<string | null> {
  const selected = await open({
    directory: true,
    multiple: false,
    title: "选择下载目录",
  });

  return typeof selected === "string" ? selected : null;
}
