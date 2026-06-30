export interface AppConfig {
  defaultDownloadDir: string;
  maxConcurrentDownloads: number;
  downloadLimit: number;
  uploadLimit: number;
}

export interface UiPreferences {
  taskTableColumnWidths: Record<string, number>;
}
