import { createPinia, setActivePinia } from "pinia";
import { NSwitch } from "naive-ui";
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
  changeAuthProtection: vi.fn(),
}));

import { changeAuthPassword, changeAuthProtection } from "../services/authService";
import { useAuthStore } from "../stores/authStore";
import { flushPromises, mountWithPinia } from "../../../test/mount";
import WebAuthSettings from "./WebAuthSettings.vue";

const mockedPassword = vi.mocked(changeAuthPassword);
const mockedProtection = vi.mocked(changeAuthProtection);

describe("WebAuthSettings", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.clearAllMocks();
  });

  it("changes the password only after validating the protected form", async () => {
    const { wrapper } = mountReadySettings();
    await wrapper.findAll("button").find((button) => button.text() === "修改密码")!.trigger("click");
    const passwordInputs = wrapper.findAll('input[type="password"]');
    await passwordInputs[0]!.setValue("current password");
    await passwordInputs[1]!.setValue("new password value");
    await passwordInputs[2]!.setValue("new password value");
    mockedPassword.mockResolvedValueOnce(status({ csrfToken: "next-csrf" }));

    await wrapper.get("form").trigger("submit");
    await flushPromises();

    expect(mockedPassword).toHaveBeenCalledWith({
      currentPassword: "current password",
      newPassword: "new password value",
    });
    expect(useAuthStore().csrfToken).toBe("next-csrf");
  });

  it("keeps the protection switch controlled until confirmation succeeds", async () => {
    const { wrapper } = mountReadySettings();
    const authStore = useAuthStore();
    const protectionSwitch = wrapper.findComponent(NSwitch);

    protectionSwitch.vm.$emit("update:value", false);
    await wrapper.vm.$nextTick();
    expect(authStore.enabled).toBe(true);
    const passwordInput = wrapper.get('input[type="password"]');
    await passwordInput.setValue("current password");
    mockedProtection.mockResolvedValueOnce(status({ enabled: false, csrfToken: "anonymous-csrf" }));
    await wrapper.get("form").trigger("submit");
    await flushPromises();

    expect(mockedProtection).toHaveBeenCalledWith({ enabled: false, currentPassword: "current password" });
    expect(authStore.enabled).toBe(false);
    expect(authStore.csrfToken).toBe("anonymous-csrf");
  });

  it("keeps the original protection state when confirmation fails", async () => {
    const { wrapper } = mountReadySettings();
    const authStore = useAuthStore();
    wrapper.findComponent(NSwitch).vm.$emit("update:value", false);
    await wrapper.vm.$nextTick();
    await wrapper.get('input[type="password"]').setValue("wrong password");
    mockedProtection.mockRejectedValueOnce(new Error("invalid credentials"));

    await wrapper.get("form").trigger("submit");
    await flushPromises();

    expect(authStore.enabled).toBe(true);
    expect(wrapper.get('[data-test="protection-error"]').text()).toContain("invalid credentials");
    expect(wrapper.find("form").exists()).toBe(true);
  });

  it("locks both security modals while submitting", async () => {
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

    const protection = mountReadySettings();
    protection.wrapper.findComponent(NSwitch).vm.$emit("update:value", false);
    await protection.wrapper.vm.$nextTick();
    useAuthStore().isSubmitting = true;
    await protection.wrapper.vm.$nextTick();

    const protectionModal = protection.wrapper.get('[data-test="n-modal"]');
    expect(protectionModal.attributes("data-mask-closable")).toBe("false");
    expect(protectionModal.attributes("data-closable")).toBe("false");
    expect(protection.wrapper.findAll("button").find((button) => button.text() === "取消")?.attributes("disabled")).toBeDefined();
  });

  it("prevents anonymous management contexts from opening privileged operations", async () => {
    const { wrapper } = mountReadySettings(false);
    const buttons = wrapper.findAll("button");
    expect(buttons.find((button) => button.text() === "修改密码")!.attributes("disabled")).toBeDefined();
    expect(wrapper.findComponent(NSwitch).props("disabled")).toBe(true);
    expect(wrapper.text()).toContain("匿名管理上下文");
  });
});

function mountReadySettings(authenticated = true) {
  const pinia = createPinia();
  setActivePinia(pinia);
  const authStore = useAuthStore();
  authStore.handleUnauthorizedStatus(status({ enabled: authenticated, authenticated, csrfToken: "csrf" }));
  return mountWithPinia(WebAuthSettings, {
    pinia,
    global: { stubs: { teleport: true } },
  });
}

function status(overrides: Partial<ReturnType<typeof baseStatus>> = {}) {
  return { ...baseStatus(), ...overrides };
}

function baseStatus() {
  return { setupRequired: false, enabled: true, authenticated: true, csrfToken: "csrf" as string | null };
}
