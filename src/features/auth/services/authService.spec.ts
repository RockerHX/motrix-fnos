import { beforeEach, describe, expect, it, vi } from "vitest";
import { httpGet, httpGetBlob, httpPost, httpPut } from "../../../services/http";
import {
  changeAuthPassword,
  getAuthStatus,
  loginAuth,
  downloadLoginDiagnostic,
  logoutAuth,
  setupAuth,
} from "./authService";

vi.mock("../../../services/http", () => ({
  httpGet: vi.fn(),
  httpGetBlob: vi.fn(),
  httpPost: vi.fn(),
  httpPut: vi.fn(),
}));

describe("authService", () => {
  beforeEach(() => vi.clearAllMocks());

  it("uses public endpoints without global 401 handling", () => {
    getAuthStatus();
    setupAuth("new password value");
    loginAuth("current password");
    expect(httpGet).toHaveBeenCalledWith("/api/auth/status", { handleUnauthorized: false });
    expect(httpPost).toHaveBeenCalledWith(
      "/api/auth/setup",
      { password: "new password value" },
      { handleUnauthorized: false, includeAuth: false },
    );
    expect(httpPost).toHaveBeenCalledWith(
      "/api/auth/login",
      { password: "current password" },
      { handleUnauthorized: false, includeAuth: false },
    );
  });

  it("maps privileged password changes to their contract", () => {
    logoutAuth();
    changeAuthPassword({ currentPassword: "old", newPassword: "new password value" });
    expect(httpPost).toHaveBeenCalledWith("/api/auth/logout");
    expect(httpPut).toHaveBeenCalledWith("/api/auth/password", {
      currentPassword: "old",
      newPassword: "new password value",
    }, { handleUnauthorized: false, includeAuth: false });
  });

  it("downloads the unauthenticated login diagnostic endpoint", () => {
    downloadLoginDiagnostic();
    expect(httpGetBlob).toHaveBeenCalledWith("/api/auth/login-diagnostic", {
      handleUnauthorized: false,
      includeAuth: false,
    });
  });
});
