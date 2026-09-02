import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { useDebugLogStore } from "../../diagnostics/stores/debugLogStore";
import { useSettingsStore } from "../../settings/stores/settingsStore";
import { useJsonRpcTokenStore } from "../../settings/stores/jsonRpcTokenStore";
import { useTaskStore } from "../../tasks/stores/taskStore";
import { getAuthStatus, loginAuth, logoutAuth, setupAuth } from "../services/authService";
import { useAuthStore } from "./authStore";

vi.mock("../services/authService", () => ({
  getAuthStatus: vi.fn(),
  setupAuth: vi.fn(),
  loginAuth: vi.fn(),
  logoutAuth: vi.fn(),
  changeAuthPassword: vi.fn(),
  changeAuthProtection: vi.fn(),
}));

const mockedStatus = vi.mocked(getAuthStatus);
const mockedSetup = vi.mocked(setupAuth);
const mockedLogin = vi.mocked(loginAuth);
const mockedLogout = vi.mocked(logoutAuth);

describe("authStore", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.clearAllMocks();
    localStorage.clear();
  });

  it("maps server status to setup, login and ready phases without persisting a status response", async () => {
    const store = useAuthStore();
    mockedStatus.mockResolvedValueOnce(status({ setupRequired: true }));
    await store.initialize();
    expect(store.phase).toBe("setup");
    expect(store.accessToken).toBeNull();

    mockedStatus.mockResolvedValueOnce(status({ authenticated: false, accessToken: null }));
    await store.initialize();
    expect(store.phase).toBe("login");

    mockedStatus.mockResolvedValueOnce(status({ authenticated: true }));
    await store.initialize();
    expect(store.phase).toBe("ready");
    expect(store.accessToken).toBeNull();
    expect(localStorage.length).toBe(0);
    expect(sessionStorage.length).toBe(0);
  });

  it("supports setup, login and logout while clearing sensitive stores", async () => {
    const store = useAuthStore();
    mockedSetup.mockResolvedValueOnce(status({ authenticated: true, accessToken: "setup-jwt" }));
    mockedStatus.mockResolvedValueOnce(status({ authenticated: true }));
    await store.setup("new password value");
    expect(store.phase).toBe("ready");

    mockedLogin.mockResolvedValueOnce(status({ authenticated: true, accessToken: "login-jwt" }));
    mockedStatus.mockResolvedValueOnce(status({ authenticated: true }));
    await store.login("current password");
    expect(store.accessToken).toBe("login-jwt");

    const taskStore = useTaskStore();
    taskStore.tasks = [{ id: 1 } as never];
    const settingsStore = useSettingsStore();
    settingsStore.accessiblePaths = ["/downloads"];
    const debugStore = useDebugLogStore();
    debugStore.logs = [{ id: 1 } as never];
    const tokenStore = useJsonRpcTokenStore();
    tokenStore.draftToken = "raw-token";
    mockedLogout.mockResolvedValueOnce(undefined);
    await store.logout();
    expect(store.phase).toBe("login");
    expect(store.accessToken).toBeNull();
    expect(taskStore.tasks).toEqual([]);
    expect(settingsStore.accessiblePaths).toEqual([]);
    expect(debugStore.logs).toEqual([]);
    expect(tokenStore.draftToken).toBe("");
  });

  it("keeps a newly issued token in memory when localStorage is blocked", async () => {
    const store = useAuthStore();
    const setItem = vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => {
      throw new Error("storage blocked");
    });
    mockedLogin.mockResolvedValueOnce(status({ authenticated: true, accessToken: "memory-jwt" }));
    mockedStatus.mockResolvedValueOnce(status({ authenticated: true }));

    await store.login("current password");

    expect(store.phase).toBe("ready");
    expect(store.accessToken).toBe("memory-jwt");
    expect(store.localStorageAvailable).toBe(false);
    setItem.mockRestore();
  });

  it("does not enter the business UI when the new access token is not usable", async () => {
    const store = useAuthStore();
    mockedLogin.mockResolvedValueOnce(status({ authenticated: true, accessToken: "login-jwt" }));
    mockedStatus.mockResolvedValueOnce(status({ authenticated: false, accessToken: null }));

    await expect(store.login("current password")).rejects.toThrow("管理访问令牌");
    expect(store.phase).toBe("login");
    expect(store.authenticated).toBe(false);
    expect(store.accessToken).toBeNull();
  });

  it("rechecks status after a business 401 and clears sensitive state", async () => {
    const store = useAuthStore();
    const taskStore = useTaskStore();
    taskStore.tasks = [{ id: 1 } as never];
    mockedStatus.mockResolvedValueOnce(status({ authenticated: false, accessToken: null }));

    await store.handleUnauthorized();

    expect(store.phase).toBe("login");
    expect(taskStore.tasks).toEqual([]);
  });

  it("applies an SSE auth probe result and clears all sensitive state", () => {
    const store = useAuthStore();
    store.handleUnauthorizedStatus(status({ authenticated: true, accessToken: "jwt" }));
    const taskStore = useTaskStore();
    taskStore.tasks = [{ id: 1 } as never];
    const settingsStore = useSettingsStore();
    settingsStore.accessiblePaths = ["/downloads"];
    const debugStore = useDebugLogStore();
    debugStore.logs = [{ id: 1 } as never];

    store.handleUnauthorizedStatus(status({ authenticated: false, accessToken: null }));

    expect(store.phase).toBe("login");
    expect(store.accessToken).toBeNull();
    expect(taskStore.tasks).toEqual([]);
    expect(settingsStore.accessiblePaths).toEqual([]);
    expect(debugStore.logs).toEqual([]);
  });

  it("clears sensitive state when a status refresh leaves ready", async () => {
    const store = useAuthStore();
    mockedStatus.mockResolvedValueOnce(status({ authenticated: true }));
    await store.initialize();
    const taskStore = useTaskStore();
    taskStore.tasks = [{ id: 1 } as never];
    mockedStatus.mockResolvedValueOnce(status({ setupRequired: true }));

    await store.refreshStatus();

    expect(store.phase).toBe("setup");
    expect(taskStore.tasks).toEqual([]);
  });
});

function status(overrides: Partial<ReturnType<typeof baseStatus>> = {}) {
  return { ...baseStatus(), ...overrides };
}

function baseStatus() {
  return {
    setupRequired: false,
    enabled: true,
    authenticated: false,
    accessToken: null as string | null,
  };
}
