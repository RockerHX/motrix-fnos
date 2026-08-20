import { beforeEach, describe, expect, it, vi } from "vitest";
import { httpGet, httpPost, httpPut } from "../../../services/http";
import {
  changeAuthPassword,
  changeAuthProtection,
  getAuthStatus,
  loginAuth,
  logoutAuth,
  setupAuth,
} from "./authService";

vi.mock("../../../services/http", () => ({
  httpGet: vi.fn(),
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
      { handleUnauthorized: false },
    );
    expect(httpPost).toHaveBeenCalledWith(
      "/api/auth/login",
      { password: "current password" },
      { handleUnauthorized: false },
    );
  });

  it("maps privileged auth operations to their contracts", () => {
    logoutAuth();
    changeAuthPassword({ currentPassword: "old", newPassword: "new password value" });
    changeAuthProtection({ enabled: false, currentPassword: "old" });
    expect(httpPost).toHaveBeenCalledWith("/api/auth/logout");
    expect(httpPut).toHaveBeenCalledWith("/api/auth/password", {
      currentPassword: "old",
      newPassword: "new password value",
    });
    expect(httpPut).toHaveBeenCalledWith("/api/auth/protection", {
      enabled: false,
      currentPassword: "old",
    });
  });
});
