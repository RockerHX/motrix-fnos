export interface Aria2ConfigStatus {
  configured: boolean;
  path?: string | null;
  pathExists: boolean;
  rpcHost: string;
  rpcPort: number;
  rpcSecretConfigured: boolean;
}

export interface Aria2ProcessStatus {
  running: boolean;
  pid?: number | null;
  message: string;
}

export interface Aria2RpcStatus {
  connected: boolean;
  version?: string | null;
  message: string;
}
