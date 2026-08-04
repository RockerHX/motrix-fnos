import { reactive, ref } from "vue";
import { beforeEach, describe, expect, it, vi } from "vitest";

const isMobileLayout = ref(false);
const mockCloseDialog = vi.fn();
const mockSubmitCreateTask = vi.fn();
const mockUseTaskCreateForm = vi.fn();

vi.mock("../../../app/composables/useMobileLayout", () => ({
  useMobileLayout: () => ({
    isMobileLayout,
  }),
}));

vi.mock("../composables/useTaskCreateForm", () => ({
  useTaskCreateForm: (...args: unknown[]) => mockUseTaskCreateForm(...args),
}));

vi.mock("naive-ui", async () => {
  const { defineComponent, h } = await import("vue");
  const slotStub = (name: string) =>
    defineComponent({
      name,
      setup(_, { slots }) {
        return () => h("div", { "data-test": name }, slots.default?.());
      },
    });

  return {
    NAlert: slotStub("n-alert"),
    NCard: defineComponent({
      name: "NCardStub",
      setup(_, { slots }) {
        return () =>
          h("div", { "data-test": "n-card" }, [
            ...(slots.header?.() ?? []),
            ...(slots["header-extra"]?.() ?? []),
            ...(slots.default?.() ?? []),
          ]);
      },
    }),
    NCollapse: slotStub("n-collapse"),
    NCollapseItem: slotStub("n-collapse-item"),
    NForm: defineComponent({
      name: "NFormStub",
      emits: ["submit"],
      setup(_, { emit, slots }) {
        return () =>
          h(
            "form",
            {
              "data-test": "n-form",
              onSubmit: (event: Event) => emit("submit", event),
            },
            slots.default?.(),
          );
      },
    }),
    NFormItem: slotStub("n-form-item"),
    NGi: slotStub("n-gi"),
    NGrid: slotStub("n-grid"),
    NInput: defineComponent({
      name: "NInputStub",
      props: {
        value: {
          type: [String, Number],
          default: "",
        },
      },
      emits: ["update:value"],
      setup(props, { emit }) {
        return () =>
          h("input", {
            value: props.value,
            onInput: (event: Event) => emit("update:value", (event.target as HTMLInputElement).value),
          });
      },
    }),
    NInputNumber: defineComponent({
      name: "NInputNumberStub",
      props: {
        value: {
          type: Number,
          default: 0,
        },
      },
      emits: ["update:value"],
      setup(props, { emit }) {
        return () =>
          h("input", {
            type: "number",
            value: props.value,
            onInput: (event: Event) => emit("update:value", Number((event.target as HTMLInputElement).value)),
          });
      },
    }),
    NModal: defineComponent({
      name: "NModalStub",
      props: {
        show: {
          type: Boolean,
          default: false,
        },
        maskClosable: {
          type: Boolean,
          default: true,
        },
      },
      emits: ["update:show"],
      setup(props, { emit, slots }) {
        return () =>
          props.show
            ? h(
                "div",
                {
                  "data-test": "n-modal",
                  "data-mask-closable": String(props.maskClosable),
                  onClick: (event: MouseEvent) => {
                    if (event.target === event.currentTarget && props.maskClosable) {
                      emit("update:show", false);
                    }
                  },
                },
                slots.default?.(),
              )
            : null;
      },
    }),
    NSelect: defineComponent({
      name: "NSelectStub",
      inheritAttrs: false,
      props: {
        value: {
          type: String,
          default: "",
        },
        options: {
          type: Array,
          default: () => [],
        },
      },
      emits: ["update:value"],
      setup(props, { emit }) {
        return () =>
          h(
            "select",
            {
              value: props.value,
              onChange: (event: Event) => emit("update:value", (event.target as HTMLSelectElement).value),
            },
            (props.options as Array<{ label: string; value: string }>).map((option) =>
              h("option", { value: option.value }, option.label),
            ),
          );
      },
    }),
    NSpace: slotStub("n-space"),
    NSwitch: defineComponent({
      name: "NSwitchStub",
      inheritAttrs: false,
      props: {
        value: { type: Boolean, default: false },
        disabled: { type: Boolean, default: false },
        loading: { type: Boolean, default: false },
      },
      emits: ["update:value"],
      setup(props, { emit, attrs }) {
        return () =>
          h("button", {
            ...attrs,
            type: "button",
            role: "switch",
            "aria-checked": String(props.value),
            disabled: props.disabled,
            "data-loading": String(props.loading),
            onClick: () => {
              if (!props.disabled) emit("update:value", !props.value);
            },
          });
      },
    }),
    NTabPane: slotStub("n-tab-pane"),
    NTabs: slotStub("n-tabs"),
    NUpload: defineComponent({
      name: "NUploadStub",
      emits: ["change", "remove"],
      setup(_, { slots }) {
        return () => h("div", { "data-test": "n-upload" }, slots.default?.());
      },
    }),
    NButton: defineComponent({
      name: "NButtonStub",
      props: {
        disabled: {
          type: Boolean,
          default: false,
        },
        attrType: {
          type: String,
          default: "button",
        },
      },
      emits: ["click"],
      setup(props, { emit, slots, attrs }) {
        return () =>
          h(
            "button",
            {
              ...attrs,
              type: props.attrType,
              disabled: props.disabled,
              onClick: (event: MouseEvent) => {
                event.stopPropagation();
                if (!props.disabled) {
                  emit("click", event);
                }
              },
            },
            slots.default?.(),
          );
      },
    }),
  };
});

import TaskCreateDialog from "./TaskCreateDialog.vue";
import { flushPromises, mountWithPinia } from "../../../test/mount";

describe("TaskCreateDialog", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockUseTaskCreateForm.mockReturnValue(createComposableState());
  });

  it("passes show ref into useTaskCreateForm and renders modal content", () => {
    const { wrapper } = mountWithPinia(TaskCreateDialog, {
      props: {
        show: true,
      },
    });

    const call = mockUseTaskCreateForm.mock.calls[0]?.[0] as { show: { value: boolean } };
    expect(call.show.value).toBe(true);
    expect(wrapper.find('[data-test="n-modal"]').exists()).toBe(true);
    expect(wrapper.text()).toContain("新建下载任务");
  });

  it("keeps dialog actions outside the scrollable form fields", () => {
    const { wrapper } = mountWithPinia(TaskCreateDialog, {
      props: {
        show: true,
      },
    });

    const form = wrapper.get('[data-test="n-form"]');
    const fields = form.get(".task-create-fields");
    const actions = form.get(".dialog-actions");

    expect(fields.find(".task-create-tabs").exists()).toBe(true);
    expect(fields.find(".dialog-actions").exists()).toBe(false);
    expect(actions.element.parentElement).toBe(form.element);
  });

  it("blocks mask and close controls while creating or exiting", async () => {
    for (const stateOptions of [{ isCreating: true }, { isRuntimeExiting: true }]) {
      const state = createComposableState(stateOptions);
      mockUseTaskCreateForm.mockReturnValue(state);
      const { wrapper } = mountWithPinia(TaskCreateDialog, {
        props: { show: true },
      });

      const modal = wrapper.get('[data-test="n-modal"]');
      expect(modal.attributes("data-mask-closable")).toBe("false");
      expect(wrapper.get('button[aria-label="关闭"]').attributes("disabled")).toBeDefined();
      expect(wrapper.findAll("button").find((button) => button.text() === "取消")?.attributes("disabled")).toBeDefined();

      await modal.trigger("click");
      await wrapper.get('button[aria-label="关闭"]').trigger("click");
      await wrapper.findAll("button").find((button) => button.text() === "取消")!.trigger("click");

      expect(mockCloseDialog).not.toHaveBeenCalled();
    }
  });

  it("allows a mask close when task creation is unlocked", async () => {
    const state = createComposableState();
    mockUseTaskCreateForm.mockReturnValue(state);
    const { wrapper } = mountWithPinia(TaskCreateDialog, {
      props: { show: true },
    });

    const modal = wrapper.get('[data-test="n-modal"]');
    expect(modal.attributes("data-mask-closable")).toBe("true");
    await modal.trigger("click");

    expect(mockCloseDialog).toHaveBeenCalledOnce();
  });

  it("binds composable state and handlers to the rendered dialog", async () => {
    mockUseTaskCreateForm.mockReturnValue(
      createComposableState({
        canSubmit: false,
        formErrorMessage: "表单错误",
        accessiblePathsError: "目录读取失败",
      }),
    );

    const { wrapper } = mountWithPinia(TaskCreateDialog, {
      props: {
        show: true,
      },
    });

    expect(wrapper.text()).toContain("表单错误");
    expect(wrapper.text()).toContain("目录读取失败");

    const submitButton = wrapper.findAll("button").find((button) => button.text() === "开始下载");
    expect(submitButton?.attributes("disabled")).toBeDefined();

    await wrapper.get('button[aria-label="关闭"]').trigger("click");
    await wrapper.findAll("button").find((button) => button.text() === "取消")!.trigger("click");
    await wrapper.get('[data-test="n-form"]').trigger("submit");
    await flushPromises();

    expect(mockCloseDialog).toHaveBeenCalledTimes(2);
    expect(mockSubmitCreateTask).toHaveBeenCalledTimes(1);
  });

  it("controls proxy selection and opens settings when no proxy is configured", async () => {
    const configuredState = createComposableState();
    mockUseTaskCreateForm.mockReturnValue(configuredState);
    const { wrapper } = mountWithPinia(TaskCreateDialog, { props: { show: true } });

    const proxySwitch = wrapper.get('button[role="switch"]');
    expect(proxySwitch.attributes("disabled")).toBeUndefined();
    await proxySwitch.trigger("click");
    expect(configuredState.form.useProxy).toBe(true);

    const unavailableState = createComposableState({ canUseProxy: false, isProxyConfigured: false });
    mockUseTaskCreateForm.mockReturnValue(unavailableState);
    const { wrapper: unavailableWrapper } = mountWithPinia(TaskCreateDialog, { props: { show: true } });
    expect(unavailableWrapper.get('button[role="switch"]').attributes("disabled")).toBeDefined();
    const unavailableStateRow = unavailableWrapper.get('[data-test="proxy-unavailable-state"]');
    expect(unavailableStateRow.text()).toContain("尚未配置下载代理");
    expect(unavailableWrapper.find(".proxy-state-alert").exists()).toBe(false);

    await unavailableWrapper.findAll("button").find((button) => button.text() === "前往设置")!.trigger("click");
    expect(unavailableState.openProxySettings).toHaveBeenCalledOnce();

    const lastCall = mockUseTaskCreateForm.mock.calls[mockUseTaskCreateForm.mock.calls.length - 1];
    const options = lastCall?.[0] as { onOpenProxySettings: () => void };
    options.onOpenProxySettings();
    expect(unavailableWrapper.emitted("openSettings")).toHaveLength(1);

    const loadFailedState = createComposableState({ canUseProxy: false, hasProxyStatusError: true });
    mockUseTaskCreateForm.mockReturnValue(loadFailedState);
    const { wrapper: loadFailedWrapper } = mountWithPinia(TaskCreateDialog, { props: { show: true } });
    expect(loadFailedWrapper.get('[data-test="proxy-unavailable-state"]').text()).toContain("无法读取下载代理状态");
  });

  it("selects and removes torrent files through Naive UI upload", async () => {
    const composableState = createComposableState();
    composableState.activeInputType.value = "torrent";
    mockUseTaskCreateForm.mockReturnValue(composableState);

    const { wrapper } = mountWithPinia(TaskCreateDialog, {
      props: {
        show: true,
      },
    });
    const torrentFile = new File(["torrent"], "demo.torrent", { type: "application/x-bittorrent" });
    const upload = wrapper.getComponent({ name: "NUploadStub" });

    upload.vm.$emit("change", {
      file: {
        id: "demo.torrent-1",
        name: torrentFile.name,
        status: "pending",
        file: torrentFile,
      },
      fileList: [],
      event: undefined,
    });
    await flushPromises();

    expect(composableState.selectTorrentFile).toHaveBeenCalledWith(torrentFile);

    upload.vm.$emit("remove", {
      file: {
        id: "demo.torrent-1",
        name: torrentFile.name,
        status: "removed",
        file: torrentFile,
      },
      fileList: [],
      index: 0,
    });
    await flushPromises();

    expect(composableState.selectTorrentFile).toHaveBeenCalledWith(null);
  });
});

function createComposableState(overrides: {
  canSubmit?: boolean;
  formErrorMessage?: string;
  accessiblePathsError?: string;
  isCreating?: boolean;
  isRuntimeExiting?: boolean;
  canUseProxy?: boolean;
  isProxyConfigured?: boolean;
  hasProxyStatusError?: boolean;
} = {}) {
  const taskStore = reactive({
    isCreating: overrides.isCreating ?? false,
    isRuntimeExiting: overrides.isRuntimeExiting ?? false,
  });

  return {
    taskStore,
    form: reactive({
      url: "",
      batchUrls: "",
      magnet: "",
      torrentFile: null,
      fileName: "",
      saveDir: "",
      startMode: "now",
      category: "默认",
      connections: 16,
      downloadLimitKb: 0,
      useProxy: false,
    }),
    activeInputType: ref("url"),
    formErrorMessage: ref(overrides.formErrorMessage ?? ""),
    batchFailedItems: ref([]),
    accessiblePaths: ref<string[]>(["/downloads"]),
    isLoadingAccessiblePaths: ref(false),
    accessiblePathsError: ref(overrides.accessiblePathsError ?? ""),
    urlFeedback: ref<string | undefined>(undefined),
    urlValidationStatus: ref<string | undefined>(undefined),
    magnetFeedback: ref<string | undefined>(undefined),
    magnetValidationStatus: ref<string | undefined>(undefined),
    accessiblePathOptions: ref([{ label: "/downloads", value: "/downloads" }]),
    canSubmit: ref(overrides.canSubmit ?? true),
    isMaskClosable: ref(!taskStore.isCreating && !taskStore.isRuntimeExiting),
    isProxyConfigured: ref(overrides.isProxyConfigured ?? true),
    isLoadingProxyStatus: ref(false),
    hasProxyStatusError: ref(overrides.hasProxyStatusError ?? false),
    canUseProxy: ref(overrides.canUseProxy ?? true),
    selectTorrentFile: vi.fn(),
    submitCreateTask: mockSubmitCreateTask,
    closeDialog: () => {
      if (!taskStore.isCreating && !taskStore.isRuntimeExiting) {
        mockCloseDialog();
      }
    },
    openProxySettings: vi.fn(),
  };
}
