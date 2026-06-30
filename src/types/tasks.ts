export type DownloadTaskStatus = "pending" | "active" | "paused" | "complete" | "error" | "removed";

export interface DownloadTask {
  id: number;
  url: string;
  fileName: string;
  saveDir: string;
  gid?: string | null;
  status: DownloadTaskStatus;
  totalLength: number;
  completedLength: number;
  downloadSpeed: number;
  errorCode?: string | null;
  errorMessage?: string | null;
  filePath?: string | null;
  createdAt: number;
}

export interface CreateDownloadTaskRequest {
  url: string;
  fileName?: string | null;
  saveDir?: string | null;
}
