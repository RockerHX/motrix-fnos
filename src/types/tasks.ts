export type DownloadTaskStatus = "pending" | "active" | "paused" | "complete" | "error" | "removed";

export interface DownloadTask {
  id: number;
  url: string;
  fileName: string;
  saveDir?: string | null;
  gid?: string | null;
  status: DownloadTaskStatus;
  totalLength: number;
  completedLength: number;
  downloadSpeed: number;
  errorMessage?: string | null;
  createdAt: number;
}

export interface CreateDownloadTaskRequest {
  url: string;
  fileName?: string | null;
  saveDir?: string | null;
}
