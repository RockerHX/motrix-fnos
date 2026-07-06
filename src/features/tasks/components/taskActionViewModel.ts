export interface TaskActionState {
  isOperating: boolean;
  isActionDisabled: boolean;
  isRuntimeExiting: boolean;
}

export interface TaskActionPermissions {
  canPause: boolean;
  canResume: boolean;
  canRedownload: boolean;
  canDelete: boolean;
  canPermanentDelete: boolean;
}

export interface TaskActionLabels {
  details: string;
  pause: string;
  resume: string;
  redownload: string;
  delete: string;
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
  deleteTitle: string;
  deleteConfirmText: string;
  deleteFilesLabel: string;
  permanentDeleteTitle: string;
  permanentDeleteConfirmText: string;
}
