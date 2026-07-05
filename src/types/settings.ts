import type { AppLanguage } from "../i18n";

export interface AppConfig {
  defaultDownloadDir: string;
  maxConcurrentDownloads: number;
  downloadLimit: number;
  uploadLimit: number;
  autoStartEnabled: boolean;
  notificationsEnabled: boolean;
  language: AppLanguage;
  jsonRpcToken: string;
}

export interface UiPreferences {
  taskTableColumnWidths: Record<string, number>;
}
