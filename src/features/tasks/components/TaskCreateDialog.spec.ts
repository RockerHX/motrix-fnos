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

    const buttons = wrapper.findAll("button");
    expect(buttons[buttons.length - 1]?.attributes("disabled")).toBeDefined();

    await buttons[0]!.trigger("click");
    await buttons[1]!.trigger("click");
    await wrapper.get('[data-test="n-form"]').trigger("submit");
    await flushPromises();

    expect(mockCloseDialog).toHaveBeenCalledTimes(2);
    expect(mockSubmitCreateTask).toHaveBeenCalledTimes(1);
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
      proxy: "",
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
    selectTorrentFile: vi.fn(),
    submitCreateTask: mockSubmitCreateTask,
    closeDialog: () => {
      if (!taskStore.isCreating && !taskStore.isRuntimeExiting) {
        mockCloseDialog();
      }
    },
  };
}
