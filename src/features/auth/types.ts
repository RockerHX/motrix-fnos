export type AuthPhase = "loading" | "setup" | "login" | "ready" | "error";

export interface AuthStatus {
  setupRequired: boolean;
  enabled: boolean;
  authenticated: boolean;
  accessToken?: string | null;
}

export interface ChangePasswordRequest {
  currentPassword: string;
  newPassword: string;
}

export interface ChangeProtectionRequest {
  enabled: boolean;
  currentPassword: string;
}

export type AuthChannelMessage = { type: "auth-invalidated" } | { type: "auth-updated" };
