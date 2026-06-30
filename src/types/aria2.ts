export type Aria2BinarySource = "externalPath" | "sidecar";

export interface Aria2ConfigStatus {
  configured: boolean;
  path?: string | null;
  pathExists: boolean;
  binarySource: Aria2BinarySource;
  sidecarName: string;
  targetTriple: string;
  rpcHost: string;
  rpcPort: number;
  rpcSecretConfigured: boolean;
}

export interface Aria2ProcessStatus {
  running: boolean;
  pid?: number | null;
  binarySource?: Aria2BinarySource | null;
  message: string;
}

export interface Aria2RpcStatus {
  connected: boolean;
  version?: string | null;
  message: string;
}
