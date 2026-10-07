import { createPinia, setActivePinia, type Pinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises, mountWithPinia } from "../../../test/mount";
import { downloadLoginDiagnostic, getAuthStatus, loginAuth } from "../services/authService";
import { useAuthStore } from "../stores/authStore";
import AuthGate from "./AuthGate.vue";

vi.mock("../services/authService", () => ({
  getAuthStatus: vi.fn(),
  loginAuth: vi.fn(),
  downloadLoginDiagnostic: vi.fn(),
  logoutAuth: vi.fn(),
  changeAuthPassword: vi.fn(),
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

  it("shows recovery and diagnostics without an initialization or login form", async () => {
    const store = useAuthStore();
    store.phase = "unconfigured";
    const { wrapper } = mountWithPinia(AuthGate, { pinia });
    expect(wrapper.find('[data-test="auth-unconfigured"]').exists()).toBe(true);
    expect(wrapper.text()).toContain("安装向导");
    expect(wrapper.find("form").exists()).toBe(false);
    expect(wrapper.find('[data-test="auth-bootstrap-token"]').exists()).toBe(false);
    expect(wrapper.find('[data-test="auth-diagnostics"]').exists()).toBe(true);
  });

  it("validates and logs in with the installed management password", async () => {
    const store = useAuthStore();
    store.phase = "login";
    vi.mocked(loginAuth).mockResolvedValueOnce({ setupRequired: false, authenticated: true, accessToken: "login-jwt" });
    vi.mocked(getAuthStatus).mockResolvedValueOnce({ setupRequired: false, authenticated: true });
    const { wrapper } = mountWithPinia(AuthGate, { pinia });
    const input = wrapper.find('[data-test="auth-password"] input');
    expect(input.attributes("autocomplete")).toBe("current-password");
    await input.setValue("short");
    await wrapper.find("form").trigger("submit");
    expect(loginAuth).not.toHaveBeenCalled();
    await input.setValue("installed password");
    await wrapper.find("form").trigger("submit");
    await flushPromises();
    expect(loginAuth).toHaveBeenCalledWith("installed password");
    expect(store.phase).toBe("ready");
    expect(JSON.stringify(localStorage)).not.toContain("installed password");
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

  it("offers login diagnostics without requiring login", async () => {
    const store = useAuthStore();
    store.phase = "login";
    store.accessToken = "jwt-must-not-appear-in-diagnostics";
    vi.mocked(downloadLoginDiagnostic).mockResolvedValueOnce(new Blob(["zip"]));
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: { writeText },
    });
    const createObjectURL = vi.spyOn(URL, "createObjectURL").mockReturnValue("blob:test");
    const revokeObjectURL = vi.spyOn(URL, "revokeObjectURL").mockImplementation(() => undefined);
    const anchorClick = vi.spyOn(HTMLAnchorElement.prototype, "click").mockImplementation(() => undefined);
    const { wrapper } = mountWithPinia(AuthGate, { pinia });

    expect(wrapper.find('[data-test="auth-diagnostics"]').exists()).toBe(true);
    await wrapper.find('[data-test="auth-copy-diagnostic"]').trigger("click");
    const copied = writeText.mock.calls[0]?.[0] as string;
    expect(copied).toContain("访问地址：");
    expect(copied).not.toContain("jwt-must-not-appear-in-diagnostics");

    await wrapper.find('[data-test="auth-download-diagnostic"]').trigger("click");
    await flushPromises();
    expect(downloadLoginDiagnostic).toHaveBeenCalledOnce();
    expect(createObjectURL).toHaveBeenCalledOnce();
    expect(anchorClick).toHaveBeenCalledOnce();
    expect(revokeObjectURL).toHaveBeenCalledWith("blob:test");
  });

  it("shows a recoverable message when clipboard access is unavailable", async () => {
    const store = useAuthStore();
    store.phase = "login";
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: { writeText: vi.fn().mockRejectedValue(new Error("denied")) },
    });
    const { wrapper } = mountWithPinia(AuthGate, { pinia });

    await wrapper.find('[data-test="auth-copy-diagnostic"]').trigger("click");
    await flushPromises();
    expect(wrapper.find('[data-test="auth-diagnostic-error"]').text()).toContain("无法自动复制");
    expect(wrapper.find('[data-test="auth-diagnostic-text"]').element).toHaveProperty("value", expect.stringContaining("访问地址："));
  });
});
