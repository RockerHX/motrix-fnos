import { httpDelete, httpGet, httpPost, httpPostFormData, httpPut } from "../../../services/http";
import type {
  CreateBatchDownloadTasksRequest,
  CreateBatchDownloadTasksResponse,
  ConfirmDownloadTaskFilesRequest,
  CreateDownloadTaskRequest,
  CreateTorrentDownloadTaskRequest,
  DownloadTask,
} from "../../../types/tasks";

export function createDownloadTask(payload: CreateDownloadTaskRequest): Promise<DownloadTask> {
  return httpPost<DownloadTask>("/api/tasks", payload);
}

export function createBatchDownloadTasks(
  payload: CreateBatchDownloadTasksRequest,
): Promise<CreateBatchDownloadTasksResponse> {
  return httpPost<CreateBatchDownloadTasksResponse>("/api/tasks/batch", payload);
}

export function createTorrentDownloadTask(payload: CreateTorrentDownloadTaskRequest): Promise<DownloadTask> {
  const formData = new FormData();
  formData.append("torrent", payload.torrent);
  formData.append(
    "request",
    JSON.stringify({
      saveDir: payload.saveDir,
      startMode: payload.startMode,
      category: payload.category,
      advancedOptions: payload.advancedOptions,
    }),
  );

  return httpPostFormData<DownloadTask>("/api/tasks/torrent", formData);
}

export function listDownloadTasks(signal?: AbortSignal): Promise<DownloadTask[]> {
  if (signal) {
    return httpGet<DownloadTask[]>("/api/tasks", { signal });
  }
  return httpGet<DownloadTask[]>("/api/tasks");
}

export function listRemovedDownloadTasks(signal?: AbortSignal): Promise<DownloadTask[]> {
  if (signal) {
    return httpGet<DownloadTask[]>("/api/tasks?status=removed", { signal });
  }
  return httpGet<DownloadTask[]>("/api/tasks?status=removed");
}

export function pauseDownloadTask(taskId: number): Promise<DownloadTask> {
  return httpPost<DownloadTask>(`/api/tasks/${taskId}/pause`);
}

export function resumeDownloadTask(taskId: number): Promise<DownloadTask> {
  return httpPost<DownloadTask>(`/api/tasks/${taskId}/resume`);
}

export function updateDownloadTaskProxy(taskId: number, enabled: boolean): Promise<DownloadTask> {
  return httpPut<DownloadTask>(`/api/tasks/${taskId}/proxy`, { enabled });
}

export function confirmDownloadTaskFiles(
  taskId: number,
  payload: ConfirmDownloadTaskFilesRequest,
): Promise<DownloadTask> {
  return httpPost<DownloadTask>(`/api/tasks/${taskId}/confirm`, payload);
}

export function redownloadDownloadTask(taskId: number, useProxy?: boolean): Promise<DownloadTask> {
  return useProxy === undefined
    ? httpPost<DownloadTask>(`/api/tasks/${taskId}/redownload`)
    : httpPost<DownloadTask>(`/api/tasks/${taskId}/redownload`, { useProxy });
}

export function restoreDownloadTask(taskId: number, useProxy?: boolean): Promise<DownloadTask> {
  return useProxy === undefined
    ? httpPost<DownloadTask>(`/api/tasks/${taskId}/restore`)
    : httpPost<DownloadTask>(`/api/tasks/${taskId}/restore`, { useProxy });
}

export function deleteDownloadTask(taskId: number, deleteFiles: boolean): Promise<DownloadTask> {
  return httpDelete<DownloadTask>(`/api/tasks/${taskId}?deleteFiles=${deleteFiles ? "true" : "false"}`);
}

export function permanentlyDeleteDownloadTask(taskId: number): Promise<void> {
  return httpDelete<void>(`/api/tasks/${taskId}/permanent`);
}
