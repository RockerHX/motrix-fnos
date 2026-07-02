import { httpDelete, httpGet, httpPost } from "../../../services/http";
import type { CreateDownloadTaskRequest, DownloadTask } from "../../../types/tasks";

export function createDownloadTask(payload: CreateDownloadTaskRequest): Promise<DownloadTask> {
  return httpPost<DownloadTask>("/api/tasks", payload);
}

export function listDownloadTasks(): Promise<DownloadTask[]> {
  return httpGet<DownloadTask[]>("/api/tasks");
}

export function pauseDownloadTask(taskId: number): Promise<DownloadTask> {
  return httpPost<DownloadTask>(`/api/tasks/${taskId}/pause`);
}

export function resumeDownloadTask(taskId: number): Promise<DownloadTask> {
  return httpPost<DownloadTask>(`/api/tasks/${taskId}/resume`);
}

export function redownloadDownloadTask(taskId: number): Promise<DownloadTask> {
  return httpPost<DownloadTask>(`/api/tasks/${taskId}/redownload`);
}

export function deleteDownloadTask(taskId: number, deleteFiles: boolean): Promise<DownloadTask> {
  return httpDelete<DownloadTask>(`/api/tasks/${taskId}?deleteFiles=${deleteFiles ? "true" : "false"}`);
}
