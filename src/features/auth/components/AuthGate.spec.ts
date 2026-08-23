import { createPinia, setActivePinia, type Pinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises, mountWithPinia } from "../../../test/mount";
import { getAuthStatus, loginAuth, setupAuth } from "../services/authService";
import { useAuthStore } from "../stores/authStore";
import AuthGate from "./AuthGate.vue";

vi.mock("../services/authService", () => ({
  getAuthStatus: vi.fn(),
  setupAuth: vi.fn(),
  loginAuth: vi.fn(),
  logoutAuth: vi.fn(),
  changeAuthPassword: vi.fn(),
  changeAuthProtection: vi.fn(),
}));

describe("AuthGate", () => {
  let pinia: Pinia;

  beforeEach(() => {
    localStorage.clear();
    pinia = createPinia();
    setActivePinia(pinia);
    vi.clearAllMocks();
  });

  it("renders loading and retry states without business content", async () => {
    const store = useAuthStore();
    const { wrapper } = mountWithPinia(AuthGate, { pinia });
    expect(wrapper.find('[data-test="auth-loading"]').exists()).toBe(true);
    expect(wrapper.text()).not.toContain("/downloads");

    store.phase = "error";
    store.errorMessage = "服务暂不可用";
    await wrapper.vm.$nextTick();
    expect(wrapper.find('[data-test="auth-error"]').exists()).toBe(true);
    expect(wrapper.text()).toContain("服务暂不可用");
  });

  it("validates setup password and submits only matching values", async () => {
    const store = useAuthStore();
    store.phase = "setup";
    vi.mocked(setupAuth).mockResolvedValueOnce({
      setupRequired: false,
      enabled: true,
      authenticated: true,
      csrfToken: "csrf",
    });
    vi.mocked(getAuthStatus).mockResolvedValueOnce({
      setupRequired: false,
      enabled: true,
      authenticated: true,
      csrfToken: "csrf",
    });
    const { wrapper } = mountWithPinia(AuthGate, { pinia });
    const inputs = wrapper.findAll('input[type="password"]');
    await inputs[0].setValue("1234567");
    await inputs[1].setValue("1234567");
    await wrapper.find("form").trigger("submit");
    expect(wrapper.text()).toContain("8");
    expect(setupAuth).not.toHaveBeenCalled();

    await inputs[0].setValue("12345678");
    await inputs[1].setValue("12345678");
    await wrapper.find("form").trigger("submit");
    await flushPromises();
    expect(setupAuth).toHaveBeenCalledWith("12345678");
    expect(store.phase).toBe("ready");
  });

  it("submits login errors locally and persists only language preference", async () => {
    const store = useAuthStore();
    store.phase = "login";
    vi.mocked(loginAuth).mockRejectedValueOnce(new Error("管理密码无效"));
    const { wrapper } = mountWithPinia(AuthGate, { pinia });
    await wrapper.find('input[type="password"]').setValue("incorrect password");
    await wrapper.find("form").trigger("submit");
    await flushPromises();
    expect(wrapper.text()).toContain("管理密码无效");
    expect(localStorage.getItem("motrix-fnos:language")).toBeNull();
    expect(JSON.stringify(localStorage)).not.toContain("incorrect password");
  });
});
