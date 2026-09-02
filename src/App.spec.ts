import { createPinia } from "pinia";
import { mount } from "@vue/test-utils";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "./test/mount";
import { getAuthStatus } from "./features/auth/services/authService";
import type { AuthStatus } from "./features/auth/types";
import App from "./App.vue";

const runtime = vi.hoisted(() => ({ initialize: vi.fn(), dispose: vi.fn() }));
const settings = vi.hoisted(() => ({ load: vi.fn() }));
const backend = vi.hoisted(() => ({ info: vi.fn(), ping: vi.fn() }));
const platform = vi.hoisted(() => ({ initialize: vi.fn(), dispose: vi.fn() }));

vi.mock("./features/auth/services/authService", () => ({
  getAuthStatus: vi.fn(),
  setupAuth: vi.fn(),
  loginAuth: vi.fn(),
  logoutAuth: vi.fn(),
  changeAuthPassword: vi.fn(),
  changeAuthProtection: vi.fn(),
}));
vi.mock("./services/runtimeEvents", () => ({
  initializeRuntimeEvents: runtime.initialize,
  disposeRuntimeEvents: runtime.dispose,
}));
vi.mock("./features/settings/stores/settingsStore", () => ({
  useSettingsStore: () => ({ loadConfig: settings.load, clearSensitiveState: vi.fn() }),
}));
vi.mock("./services/backend", () => ({ getAppInfo: backend.info, pingBackend: backend.ping }));
vi.mock("./app/hostPlatform", () => ({
  createFnosPlatformController: () => platform,
}));
vi.mock("./app/providers/NaiveProvider.vue", () => ({ default: { template: "<div><slot /></div>" } }));
vi.mock("./features/auth/components/AuthGate.vue", () => ({ default: { template: '<div data-test="auth-gate" />' } }));
vi.mock("./views/MainWindow.vue", () => ({ default: { template: '<div data-test="main-window" />' } }));

describe("App auth bootstrap", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    settings.load.mockResolvedValue(undefined);
    backend.info.mockResolvedValue({});
    backend.ping.mockResolvedValue({});
    platform.initialize.mockResolvedValue(undefined);
  });

  it("does not mount or initialize business features before auth is ready", async () => {
    const deferred = createDeferred<AuthStatus>();
    vi.mocked(getAuthStatus).mockReturnValueOnce(deferred.promise);
    const wrapper = mount(App, { global: { plugins: [createPinia()] } });
    expect(wrapper.find('[data-test="auth-gate"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="main-window"]').exists()).toBe(false);
    expect(settings.load).not.toHaveBeenCalled();
    expect(runtime.initialize).not.toHaveBeenCalled();
    expect(platform.initialize).toHaveBeenCalledOnce();

    deferred.resolve({ setupRequired: false, enabled: true, authenticated: true });
    await flushPromises();
    expect(wrapper.find('[data-test="main-window"]').exists()).toBe(true);
    expect(settings.load).toHaveBeenCalledOnce();
    expect(runtime.initialize).toHaveBeenCalledOnce();
    wrapper.unmount();
    expect(platform.dispose).toHaveBeenCalledOnce();
  });
});

function createDeferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((resolvePromise) => (resolve = resolvePromise));
  return { promise, resolve };
}
