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
      },
      setup(props, { slots }) {
        return () => (props.show ? h("div", { "data-test": "n-modal" }, slots.default?.()) : null);
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
});

function createComposableState(overrides: {
  canSubmit?: boolean;
  formErrorMessage?: string;
  accessiblePathsError?: string;
} = {}) {
  return {
    taskStore: reactive({
      isCreating: false,
      isRuntimeExiting: false,
    }),
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
    isMaskClosable: ref(true),
    selectTorrentFile: vi.fn(),
    submitCreateTask: mockSubmitCreateTask,
    closeDialog: mockCloseDialog,
  };
}
