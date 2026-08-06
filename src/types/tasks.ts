export type DownloadTaskStatus = "pending" | "active" | "paused" | "complete" | "error" | "removed";
export type DownloadTaskSourceType = "url" | "torrent" | "magnet";
export type DownloadTaskStartMode = "now" | "paused";

export interface CreateTaskAdvancedOptions {
  connections?: number | null;
  downloadLimitKb?: number | null;
  useProxy?: boolean | null;
  proxy?: string | null;
}

export interface DownloadTask {
  id: number;
  url: string;
  /** Older servers may omit this field; the UI falls back to URL inference. */
  sourceType?: DownloadTaskSourceType;
  fileName: string;
  saveDir: string;
  /** App-owned outer directory for BT tasks; older servers may omit it. */
  ownedTaskDir?: string | null;
  category: string;
  gid?: string | null;
  status: DownloadTaskStatus;
  totalLength: number;
  completedLength: number;
  downloadSpeed: number;
  errorCode?: string | null;
  errorMessage?: string | null;
  filePath?: string | null;
  useProxy: boolean;
  confirmationRequired: boolean;
  files: DownloadTaskFile[];
  createdAt: number;
  updatedAt: number;
}

export interface DownloadTaskFile {
  index: number;
  path: string;
  name: string;
  length: number;
  completedLength: number;
  selected: boolean;
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

export interface CreateTorrentDownloadTaskRequest {
  torrent: File;
  saveDir: string;
  startMode?: DownloadTaskStartMode;
  category?: string | null;
  advancedOptions?: CreateTaskAdvancedOptions;
}

export interface ConfirmDownloadTaskFilesRequest {
  selectedFileIndexes: number[];
}
