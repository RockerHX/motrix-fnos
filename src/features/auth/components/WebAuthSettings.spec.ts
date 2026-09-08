import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { nextTick } from "vue";

vi.mock("naive-ui", async () => {
  const actual = await vi.importActual<typeof import("naive-ui")>("naive-ui");
  const { defineComponent, h } = await import("vue");
  const NModal = defineComponent({
    name: "NModalStage4Stub",
    props: {
      show: { type: Boolean, default: false },
      maskClosable: { type: Boolean, default: true },
      closable: { type: Boolean, default: true },
    },
    setup(props, { slots, attrs }) {
      return () =>
        props.show
          ? h(
              "div",
              {
                ...attrs,
                "data-test": "n-modal",
                "data-mask-closable": String(props.maskClosable),
                "data-closable": String(props.closable),
              },
              slots.default?.(),
            )
          : null;
    },
  });
  return { ...actual, NModal, useMessage: () => ({ success: vi.fn(), error: vi.fn() }) };
});

vi.mock("../services/authService", () => ({
  getAuthStatus: vi.fn(),
  setupAuth: vi.fn(),
  loginAuth: vi.fn(),
  logoutAuth: vi.fn(),
  changeAuthPassword: vi.fn(),
}));

import { changeAuthPassword, getAuthStatus } from "../services/authService";
import { useAuthStore } from "../stores/authStore";
import { flushPromises, mountWithPinia } from "../../../test/mount";
import WebAuthSettings from "./WebAuthSettings.vue";

const mockedPassword = vi.mocked(changeAuthPassword);

describe("WebAuthSettings", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.clearAllMocks();
    vi.mocked(getAuthStatus).mockResolvedValue(status());
  });

  it("changes the password only after validating the protected form", async () => {
    const { wrapper } = mountReadySettings();
    await wrapper.findAll("button").find((button) => button.text() === "修改密码")!.trigger("click");
    const passwordInputs = wrapper.findAll('input[type="password"]');
    await passwordInputs[0]!.setValue("current password");
    await passwordInputs[1]!.setValue("1234567");
    await passwordInputs[2]!.setValue("1234567");

    await wrapper.get("form").trigger("submit");
    expect(wrapper.get('[data-test="password-error"]').text()).toContain("8");
    expect(mockedPassword).not.toHaveBeenCalled();

    await passwordInputs[1]!.setValue("12345678");
    await passwordInputs[2]!.setValue("12345678");
    mockedPassword.mockResolvedValueOnce(status({ accessToken: "next-jwt" }));

    await wrapper.get("form").trigger("submit");
    await flushPromises();

    expect(mockedPassword).toHaveBeenCalledWith({
      currentPassword: "current password",
      newPassword: "12345678",
    });
    expect(useAuthStore().accessToken).toBe("next-jwt");
  });

  it("does not expose a protection switch", () => {
    const { wrapper } = mountReadySettings();
    expect(wrapper.find('[data-test="auth-protection-switch"]').exists()).toBe(false);
  });

  it("locks the password modal while submitting", async () => {
    const { wrapper } = mountReadySettings();
    await wrapper.findAll("button").find((button) => button.text() === "修改密码")!.trigger("click");
    await nextTick();

    const authStore = useAuthStore();
    authStore.isSubmitting = true;
    await nextTick();

    const modal = wrapper.get('[data-test="n-modal"]');
    expect(modal.attributes("data-mask-closable")).toBe("false");
    expect(modal.attributes("data-closable")).toBe("false");
    expect(wrapper.findAll("button").find((button) => button.text() === "取消")?.attributes("disabled")).toBeDefined();
  });
});

function mountReadySettings() {
  const pinia = createPinia();
  setActivePinia(pinia);
  const authStore = useAuthStore();
  authStore.handleUnauthorizedStatus(status({ authenticated: true, accessToken: "jwt" }));
  return mountWithPinia(WebAuthSettings, {
    pinia,
    global: { stubs: { teleport: true } },
  });
}

function status(overrides: Partial<ReturnType<typeof baseStatus>> = {}) {
  return { ...baseStatus(), ...overrides };
}

function baseStatus() {
  return { setupRequired: false, authenticated: true, accessToken: "jwt" as string | null };
}
