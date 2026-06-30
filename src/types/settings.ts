export interface AppConfig {
  defaultDownloadDir: string;
  maxConcurrentDownloads: number;
  downloadLimit: number;
  uploadLimit: number;
  autoStartEnabled: boolean;
  notificationsEnabled: boolean;
}

export interface UiPreferences {
  taskTableColumnWidths: Record<string, number>;
}
