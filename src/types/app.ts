export interface AppCompatibility {
  engineGeneration: "legacy-v1" | "native-v2" | "unknown";
  mode: "normal" | "restricted";
  reasonCode: string | null;
  unfinishedTaskCount: number;
  unresolvedOperationCount: number;
  message: string;
}

export interface AppInfo {
  name: string;
  version: string;
  backendStatus: string;
  maintainer: string;
  repositoryUrl: string;
  releasePageUrl: string;
  targetArch: string;
  updateMode: "manual_fpk_or_app_center";
}

export type UpdateCheckStatus = "available" | "up_to_date" | "unavailable";

export interface ReleaseAssetInfo {
  architecture: "x86" | "arm";
  name: string;
  downloadUrl: string;
}

export interface AppUpdateCheck {
  currentVersion: string;
  latestVersion: string | null;
  hasUpdate: boolean;
  status: UpdateCheckStatus;
  releaseUrl: string | null;
  assets: ReleaseAssetInfo[];
  checkedAt: number;
  message: string;
}

export interface BackendPing {
  ok: boolean;
  message: string;
}
