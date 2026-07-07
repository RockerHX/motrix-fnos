export type DownloadTaskStatus = "pending" | "active" | "paused" | "complete" | "error" | "removed";
export type DownloadTaskSourceType = "url" | "magnet";
export type DownloadTaskStartMode = "now" | "paused";

export interface CreateTaskAdvancedOptions {
  connections?: number | null;
  downloadLimitKb?: number | null;
  proxy?: string | null;
}

export interface DownloadTask {
  id: number;
  url: string;
  fileName: string;
  saveDir: string;
  category: string;
  gid?: string | null;
  status: DownloadTaskStatus;
  totalLength: number;
  completedLength: number;
  downloadSpeed: number;
  errorCode?: string | null;
  errorMessage?: string | null;
  filePath?: string | null;
  createdAt: number;
  updatedAt: number;
}

export interface CreateDownloadTaskRequest {
  url: string;
  fileName?: string | null;
  saveDir?: string | null;
  sourceType?: DownloadTaskSourceType;
  startMode?: DownloadTaskStartMode;
  category?: string | null;
  advancedOptions?: CreateTaskAdvancedOptions;
  aria2Options?: Record<string, unknown>;
}

export interface CreateBatchDownloadTasksRequest {
  urls: string[];
  saveDir: string;
  startMode?: DownloadTaskStartMode;
  category?: string | null;
  advancedOptions?: CreateTaskAdvancedOptions;
}

export interface CreateBatchDownloadTaskFailure {
  input: string;
  message: string;
}

export interface CreateBatchDownloadTasksResponse {
  created: DownloadTask[];
  failed: CreateBatchDownloadTaskFailure[];
}
