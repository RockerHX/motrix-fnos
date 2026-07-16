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

vi.mock("../services/jsonRpcTokenService", () => ({ getJsonRpcTokenStatus: vi.fn(), updateJsonRpcToken: vi.fn() }));

import { getJsonRpcTokenStatus, updateJsonRpcToken } from "../services/jsonRpcTokenService";
import { useJsonRpcTokenStore } from "../stores/jsonRpcTokenStore";
import { flushPromises, mountWithPinia } from "../../../test/mount";
import JsonRpcTokenSettings from "./JsonRpcTokenSettings.vue";

const mockedGetStatus = vi.mocked(getJsonRpcTokenStatus);
const mockedUpdate = vi.mocked(updateJsonRpcToken);

describe("JsonRpcTokenSettings", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.clearAllMocks();
  });

  it("shows only the server mask and clears raw input after saving", async () => {
    mockedGetStatus.mockResolvedValueOnce({ configured: true, maskedToken: "••••••••abcd" });
    const { wrapper } = mountSettings();
    await flushPromises();

    expect(wrapper.text()).toContain("••••••••abcd");
    expect(wrapper.text()).not.toContain("original-token");
    const tokenStore = useJsonRpcTokenStore();
    tokenStore.draftToken = "new-raw-token";
    mockedUpdate.mockResolvedValueOnce({ configured: true, maskedToken: "••••••••oken" });
    await wrapper.vm.$nextTick();
    await wrapper.findAll("button").find((button) => button.text() === "保存 Token")!.trigger("click");
    await flushPromises();

    expect(mockedUpdate).toHaveBeenCalledWith("new-raw-token");
    expect(tokenStore.draftToken).toBe("");
    expect(wrapper.text()).toContain("••••••••oken");
  });

  it("clears through a separate confirmation and sends an empty token", async () => {
    mockedGetStatus.mockResolvedValueOnce({ configured: true, maskedToken: "••••••••abcd" });
    mockedUpdate.mockResolvedValueOnce({ configured: false, maskedToken: null });
    const { wrapper } = mountSettings();
    await flushPromises();

    await wrapper.findAll("button").find((button) => button.text() === "清除 Token")!.trigger("click");
    await wrapper.vm.$nextTick();
    const clearButtons = wrapper.findAll("button").filter((button) => button.text() === "清除 Token");
    await clearButtons[clearButtons.length - 1]!.trigger("click");
    await flushPromises();

    expect(mockedUpdate).toHaveBeenCalledWith("");
    expect(wrapper.text()).toContain("未配置");
  });

  it("locks clear confirmation while saving", async () => {
    mockedGetStatus.mockResolvedValueOnce({ configured: true, maskedToken: "••••••••abcd" });
    const { wrapper } = mountSettings();
    await flushPromises();

    await wrapper.findAll("button").find((button) => button.text() === "清除 Token")!.trigger("click");
    await nextTick();
    const tokenStore = useJsonRpcTokenStore();
    tokenStore.isSaving = true;
    await nextTick();

    const modal = wrapper.get('[data-test="n-modal"]');
    expect(modal.attributes("data-mask-closable")).toBe("false");
    expect(modal.attributes("data-closable")).toBe("false");
    expect(modal.findAll("button").find((button) => button.text() === "取消")?.attributes("disabled")).toBeDefined();
  });

  it("drops unsaved raw input when the settings dialog closes", async () => {
    mockedGetStatus.mockResolvedValueOnce({ configured: false, maskedToken: null });
    const { wrapper } = mountSettings();
    await flushPromises();
    const tokenStore = useJsonRpcTokenStore();
    tokenStore.draftToken = "temporary-token";

    await wrapper.setProps({ active: false });

    expect(tokenStore.draftToken).toBe("");
  });
});

function mountSettings() {
  const pinia = createPinia();
  setActivePinia(pinia);
  return mountWithPinia(JsonRpcTokenSettings, {
    pinia,
    props: { active: true },
    global: { stubs: { teleport: true } },
  });
}
