export interface TaskActionState {
  isOperating: boolean;
  isActionDisabled: boolean;
  isRuntimeExiting: boolean;
}

export interface TaskActionPermissions {
  canPause: boolean;
  canResume: boolean;
  canConfirmFiles: boolean;
  canRedownload: boolean;
  canDelete: boolean;
  canRestore: boolean;
  canPermanentDelete: boolean;
}

export interface TaskActionLabels {
  details: string;
  pause: string;
  resume: string;
  confirmFiles: string;
  redownload: string;
  delete: string;
  restore: string;
  permanentDelete: string;
  cancel: string;
  close: string;
}

export interface TaskActionDetailItem {
  label: string;
  value: string;
}

export interface TaskActionDetails {
  title: string;
  items: TaskActionDetailItem[];
}

export interface TaskActionConfirmTexts {
  redownloadTitle: string;
  redownloadConfirmText: string;
  restoreTitle: string;
  restoreConfirmText: string;
  deleteTitle: string;
  deleteConfirmText: string;
  deleteFilesLabel: string;
  permanentDeleteTitle: string;
  permanentDeleteConfirmText: string;
}
