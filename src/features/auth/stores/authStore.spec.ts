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
  });

  it("maps server status to setup, login and ready phases without persisting csrf", async () => {
    const store = useAuthStore();
    mockedStatus.mockResolvedValueOnce(status({ setupRequired: true }));
    await store.initialize();
    expect(store.phase).toBe("setup");
    expect(store.csrfToken).toBeNull();

    mockedStatus.mockResolvedValueOnce(status({ authenticated: false, csrfToken: null }));
    await store.initialize();
    expect(store.phase).toBe("login");

    mockedStatus.mockResolvedValueOnce(status({ authenticated: true, csrfToken: "csrf" }));
    await store.initialize();
    expect(store.phase).toBe("ready");
    expect(store.csrfToken).toBe("csrf");
    expect(localStorage.length).toBe(0);
    expect(sessionStorage.length).toBe(0);
  });

  it("supports setup, login and logout while clearing sensitive stores", async () => {
    const store = useAuthStore();
    mockedSetup.mockResolvedValueOnce(status({ authenticated: true, csrfToken: "setup-csrf" }));
    await store.setup("new password value");
    expect(store.phase).toBe("ready");

    mockedLogin.mockResolvedValueOnce(status({ authenticated: true, csrfToken: "login-csrf" }));
    await store.login("current password");
    expect(store.csrfToken).toBe("login-csrf");

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
    expect(store.csrfToken).toBeNull();
    expect(taskStore.tasks).toEqual([]);
    expect(settingsStore.accessiblePaths).toEqual([]);
    expect(debugStore.logs).toEqual([]);
    expect(tokenStore.draftToken).toBe("");
  });

  it("rechecks status after a business 401 and clears sensitive state", async () => {
    const store = useAuthStore();
    const taskStore = useTaskStore();
    taskStore.tasks = [{ id: 1 } as never];
    mockedStatus.mockResolvedValueOnce(status({ authenticated: false, csrfToken: null }));

    await store.handleUnauthorized();

    expect(store.phase).toBe("login");
    expect(taskStore.tasks).toEqual([]);
  });

  it("applies an SSE auth probe result and clears all sensitive state", () => {
    const store = useAuthStore();
    store.handleUnauthorizedStatus(status({ authenticated: true, csrfToken: "csrf" }));
    const taskStore = useTaskStore();
    taskStore.tasks = [{ id: 1 } as never];
    const settingsStore = useSettingsStore();
    settingsStore.accessiblePaths = ["/downloads"];
    const debugStore = useDebugLogStore();
    debugStore.logs = [{ id: 1 } as never];

    store.handleUnauthorizedStatus(status({ authenticated: false, csrfToken: null }));

    expect(store.phase).toBe("login");
    expect(store.csrfToken).toBeNull();
    expect(taskStore.tasks).toEqual([]);
    expect(settingsStore.accessiblePaths).toEqual([]);
    expect(debugStore.logs).toEqual([]);
  });

  it("clears sensitive state when a status refresh leaves ready", async () => {
    const store = useAuthStore();
    mockedStatus.mockResolvedValueOnce(status({ authenticated: true, csrfToken: "csrf" }));
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
    csrfToken: null as string | null,
  };
}
