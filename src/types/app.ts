export interface AppInfo {
  name: string;
  version: string;
  backendStatus: string;
}

export interface BackendPing {
  ok: boolean;
  message: string;
}
