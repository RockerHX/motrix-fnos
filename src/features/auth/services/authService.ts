import { httpGet, httpPost, httpPut } from "../../../services/http";
import type { AuthStatus, ChangePasswordRequest, ChangeProtectionRequest } from "../types";

const publicRequest = { handleUnauthorized: false } as const;

export function getAuthStatus() {
  return httpGet<AuthStatus>("/api/auth/status", publicRequest);
}

export function setupAuth(password: string) {
  return httpPost<AuthStatus>("/api/auth/setup", { password }, publicRequest);
}

export function loginAuth(password: string) {
  return httpPost<AuthStatus>("/api/auth/login", { password }, publicRequest);
}

export function logoutAuth() {
  return httpPost<void>("/api/auth/logout");
}

export function changeAuthPassword(payload: ChangePasswordRequest) {
  return httpPut<AuthStatus>("/api/auth/password", payload);
}

export function changeAuthProtection(payload: ChangeProtectionRequest) {
  return httpPut<AuthStatus>("/api/auth/protection", payload);
}
