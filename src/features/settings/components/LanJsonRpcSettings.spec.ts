import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";

const messages = vi.hoisted(() => ({ success: vi.fn(), error: vi.fn(), warning: vi.fn() }));

vi.mock("naive-ui", async () => {
  const actual = await vi.importActual<typeof import("naive-ui")>("naive-ui");
  const { defineComponent, h } = await import("vue");
  const NSwitch = defineComponent({
    name: "NSwitchStage4Stub",
    props: { value: Boolean, disabled: Boolean, loading: Boolean },
    emits: ["update:value"],
    setup(props, { emit, attrs }) {
      return () =>
        h("input", {
          ...attrs,
          type: "checkbox",
          checked: props.value,
          disabled: props.disabled,
          onChange: (event: Event) => emit("update:value", (event.target as HTMLInputElement).checked),
        });
    },
  });
  const NModal = defineComponent({
    name: "NModalStage4Stub",
    props: { show: Boolean },
    emits: ["update:show"],
    setup(props, { slots, attrs }) {
      return () => (props.show ? h("div", { ...attrs, "data-test": "n-modal" }, slots.default?.()) : null);
    },
  });
  return { ...actual, NSwitch, NModal, useMessage: () => messages };
});
import {
  getLanJsonRpcStatus,
  rotateLanJsonRpcToken,
  updateLanJsonRpcEnabled,
} from "../services/lanJsonRpcService";
import { useLanJsonRpcStore } from "../stores/lanJsonRpcStore";
import { flushPromises, mountWithPinia } from "../../../test/mount";
import LanJsonRpcSettings from "./LanJsonRpcSettings.vue";

vi.mock("../services/lanJsonRpcService", () => ({
  getLanJsonRpcStatus: vi.fn(),
  rotateLanJsonRpcToken: vi.fn(),
  updateLanJsonRpcEnabled: vi.fn(),
}));

const mockedGet = vi.mocked(getLanJsonRpcStatus);
const mockedUpdate = vi.mocked(updateLanJsonRpcEnabled);
const mockedRotate = vi.mocked(rotateLanJsonRpcToken);

describe("LanJsonRpcSettings", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.clearAllMocks();
    mockedGet.mockResolvedValue({ enabled: false, configured: false, maskedToken: null, port: 17082 });
  });

  it("shows the first issued Token once and clears it when the modal or settings closes", async () => {
    mockedUpdate.mockResolvedValueOnce({
      status: { enabled: true, configured: true, maskedToken: "••••••••oken", port: 17082 },
      issuedToken: "one-time-lan-token",
    });
    const { wrapper } = mountSettings();
    await flushPromises();

    await wrapper.get('[data-test="lan-json-rpc-switch"]').setValue(true);
    await flushPromises();
    expect(mockedUpdate).toHaveBeenCalledWith(true);
    expect(
      (wrapper.get('[data-test="lan-json-rpc-issued-token"] input').element as HTMLInputElement).value,
    ).toBe("one-time-lan-token");
    expect((wrapper.get(".app-dialog").element as HTMLElement).style.getPropertyValue("--app-dialog-width")).toBe(
      "520px",
    );

    await wrapper.findAll("button").find((button) => button.text() === "完成")!.trigger("click");
    expect(useLanJsonRpcStore().issuedToken).toBe("");
    expect(wrapper.text()).not.toContain("one-time-lan-token");

    useLanJsonRpcStore().issuedToken = "temporary-token";
    await wrapper.setProps({ active: false });
    expect(useLanJsonRpcStore().issuedToken).toBe("");
  });

  it("keeps the switch off when enabling fails", async () => {
    mockedUpdate.mockRejectedValueOnce(new Error("save failed"));
    const { wrapper } = mountSettings();
    await flushPromises();

    await wrapper.get('[data-test="lan-json-rpc-switch"]').setValue(true);
    await flushPromises();

    expect(useLanJsonRpcStore().status?.enabled).toBe(false);
    expect(wrapper.getComponent({ name: "NSwitchStage4Stub" }).props("value")).toBe(false);
  });

  it("requires confirmation before rotation and can copy the issued Token", async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, "clipboard", { configurable: true, value: { writeText } });
    mockedRotate.mockResolvedValueOnce({
      status: { enabled: false, configured: true, maskedToken: "••••••••ated", port: 17082 },
      issuedToken: "rotated-lan-token",
    });
    const { wrapper } = mountSettings();
    await flushPromises();

    await wrapper.findAll("button").find((button) => button.text() === "轮换 Token")!.trigger("click");
    expect(mockedRotate).not.toHaveBeenCalled();
    expect((wrapper.get(".app-dialog").element as HTMLElement).style.getPropertyValue("--app-dialog-width")).toBe(
      "520px",
    );
    const rotateButtons = wrapper.findAll("button").filter((button) => button.text() === "轮换 Token");
    await rotateButtons[rotateButtons.length - 1]!.trigger("click");
    await flushPromises();
    expect(mockedRotate).toHaveBeenCalledOnce();

    await wrapper.findAll("button").find((button) => button.text() === "复制")!.trigger("click");
    await flushPromises();
    expect(writeText).toHaveBeenCalledWith("rotated-lan-token");
  });

  it("explains restricted clipboard environments and selects the issued Token for manual copy", async () => {
    Object.defineProperty(navigator, "clipboard", { configurable: true, value: undefined });
    Object.defineProperty(window, "isSecureContext", { configurable: true, value: false });
    Object.defineProperty(document, "execCommand", {
      configurable: true,
      value: vi.fn(() => true),
    });
    const select = vi.spyOn(HTMLInputElement.prototype, "select");
    mockedRotate.mockResolvedValueOnce({
      status: { enabled: false, configured: true, maskedToken: "••••••••ated", port: 17082 },
      issuedToken: "manual-copy-lan-token",
    });
    const { wrapper } = mountSettings();
    await flushPromises();

    await wrapper.findAll("button").find((button) => button.text() === "轮换 Token")!.trigger("click");
    const rotateButtons = wrapper.findAll("button").filter((button) => button.text() === "轮换 Token");
    await rotateButtons[rotateButtons.length - 1]!.trigger("click");
    await flushPromises();
    select.mockClear();

    await wrapper.findAll("button").find((button) => button.text() === "复制")!.trigger("click");
    await flushPromises();

    expect(messages.warning).toHaveBeenCalledWith(
      "当前页面不是可使用剪贴板的安全顶层环境，常见原因是局域网 HTTP 或 fnOS 内嵌窗口。请手动选择内容并按 Ctrl+C / Command+C，或直接打开 Motrix HTTPS 域名。",
    );
    expect(select).toHaveBeenCalled();
    expect(document.execCommand).not.toHaveBeenCalled();
    expect(
      (wrapper.get('[data-test="lan-json-rpc-issued-token"] input').element as HTMLInputElement).value,
    ).toBe("manual-copy-lan-token");
  });

  it("shows a LAN-IP placeholder when settings is opened through a domain", async () => {
    const { wrapper } = mountSettings();
    await flushPromises();

    expect(wrapper.get('[data-test="lan-json-rpc-endpoint"]').text()).toBe(
      "http://<飞牛局域网IP>:17082/jsonrpc",
    );
    expect(wrapper.find('[data-test="copy-lan-json-rpc-endpoint"]').exists()).toBe(false);
  });
});

function mountSettings() {
  const pinia = createPinia();
  setActivePinia(pinia);
  return mountWithPinia(LanJsonRpcSettings, { pinia, props: { active: true } });
}
