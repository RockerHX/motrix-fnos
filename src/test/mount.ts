import { createPinia, setActivePinia, type Pinia } from "pinia";
import { mount, flushPromises as flushVuePromises, type MountingOptions, type VueWrapper } from "@vue/test-utils";
import {
  defineComponent,
  h,
  type Component,
  type DefineComponent,
  type PropType,
  type VNode,
} from "vue";
import { vi } from "vitest";

type MountWithPiniaOptions = MountingOptions<any> & {
  pinia?: Pinia;
};

type Listener = (event: Event) => void;
type DataTableRow = Record<string, unknown>;
type DataTableColumn = {
  key?: string;
  title?: string;
  type?: string;
  disabled?: (row: DataTableRow) => boolean;
  render?: (row: DataTableRow) => VNode | string | number | null | undefined;
};
type DataTableRowKey = string | number;

export const naiveUiStubs = {
  NAlert: defineComponent({
    name: "NAlertStub",
    setup(_, { slots }) {
      return () => h("div", { "data-test": "n-alert" }, [slots.default?.(), slots.action?.()]);
    },
  }),
  NCard: slotStub("n-card"),
  NCollapse: slotStub("n-collapse"),
  NCollapseItem: slotStub("n-collapse-item"),
  NDescriptions: slotStub("n-descriptions"),
  NDescriptionsItem: slotStub("n-descriptions-item"),
  NForm: defineComponent({
    name: "NFormStub",
    emits: ["submit"],
    setup(_, { emit, slots }) {
      return () =>
        h(
          "form",
          {
            "data-test": "n-form",
            onSubmit: (event: Event) => {
              emit("submit", event);
            },
          },
          slots.default?.(),
        );
    },
  }),
  NFormItem: slotStub("n-form-item"),
  NGi: slotStub("n-gi"),
  NGrid: slotStub("n-grid"),
  NModal: defineComponent({
    name: "NModalStub",
    props: {
      show: {
        type: Boolean,
        default: false,
      },
    },
    emits: ["update:show"],
    setup(props, { slots }) {
      return () => (props.show ? h("div", { "data-test": "n-modal" }, slots.default?.()) : null);
    },
  }),
  NDataTable: defineComponent({
    name: "NDataTableStub",
    props: {
      columns: {
        type: Array as PropType<DataTableColumn[]>,
        default: () => [],
      },
      data: {
        type: Array as PropType<DataTableRow[]>,
        default: () => [],
      },
      checkedRowKeys: {
        type: Array as PropType<DataTableRowKey[]>,
        default: () => [],
      },
      rowKey: {
        type: Function as PropType<(row: DataTableRow) => DataTableRowKey>,
        default: (row: DataTableRow) => row.key as DataTableRowKey,
      },
    },
    emits: ["update:checked-row-keys"],
    setup(props, { emit }) {
      const getRowKey = (row: DataTableRow, index: number) => props.rowKey(row) ?? index;
      const getSelectionColumn = () => props.columns.find((column) => column.type === "selection");
      const getContentColumns = () => props.columns.filter((column) => column.type !== "selection");

      return () =>
        h("div", { "data-test": "n-data-table" }, [
          h(
            "div",
            { "data-test": "n-data-table-header" },
            getContentColumns().map((column) => h("span", { key: column.key }, column.title ?? column.key)),
          ),
          ...props.data.map((row, index) => {
            const key = getRowKey(row, index);
            const selectionColumn = getSelectionColumn();
            const checked = props.checkedRowKeys.includes(key);
            const disabled = Boolean(selectionColumn?.disabled?.(row));

            return h("div", { key, "data-test": "n-data-table-row", "data-row-key": String(key) }, [
              selectionColumn
                ? h("input", {
                    type: "checkbox",
                    checked,
                    disabled,
                    onChange: (event: Event) => {
                      if (disabled) {
                        return;
                      }
                      const nextChecked = (event.target as HTMLInputElement).checked;
                      const nextKeys = nextChecked
                        ? [...new Set([...props.checkedRowKeys, key])]
                        : props.checkedRowKeys.filter((item) => item !== key);
                      emit("update:checked-row-keys", nextKeys);
                    },
                  })
                : null,
              ...getContentColumns().map((column) =>
                h(
                  "span",
                  { key: column.key, "data-column-key": column.key },
                  column.render ? (column.render(row) ?? "") : String(row[column.key ?? ""] ?? ""),
                ),
              ),
            ]);
          }),
        ]);
    },
  }),
  NSpace: slotStub("n-space"),
  NTabs: defineComponent({
    name: "NTabsStub",
    props: {
      value: {
        type: String,
        default: "",
      },
    },
    emits: ["update:value"],
    setup(_, { slots }) {
      return () => h("div", { "data-test": "n-tabs" }, slots.default?.());
    },
  }),
  NTabPane: defineComponent({
    name: "NTabPaneStub",
    props: {
      name: {
        type: String,
        default: "",
      },
      tab: {
        type: String,
        default: "",
      },
    },
    setup(props, { slots }) {
      return () =>
        h("div", { "data-test": "n-tab-pane", "data-name": props.name, "data-tab": props.tab }, slots.default?.());
    },
  }),
  NUpload: defineComponent({
    name: "NUploadStub",
    props: {
      fileList: {
        type: Array as PropType<Array<{ id: string; name: string }>>,
        default: () => [],
      },
      disabled: {
        type: Boolean,
        default: false,
      },
    },
    emits: ["change", "remove", "update:fileList"],
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
      loading: {
        type: Boolean,
        default: false,
      },
      attrType: {
        type: String as PropType<"button" | "submit" | "reset" | undefined>,
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
            "data-loading": props.loading ? "true" : "false",
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
  NCheckbox: defineComponent({
    name: "NCheckboxStub",
    props: {
      checked: {
        type: Boolean,
        default: false,
      },
    },
    emits: ["update:checked"],
    setup(props, { emit, slots }) {
      return () =>
        h("label", { "data-test": "n-checkbox" }, [
          h("input", {
            type: "checkbox",
            checked: props.checked,
            onChange: (event: Event) => {
              const target = event.target as HTMLInputElement;
              emit("update:checked", target.checked);
            },
          }),
          slots.default?.(),
        ]);
    },
  }),
  NInput: defineComponent({
    name: "NInputStub",
    props: {
      value: {
        type: String,
        default: "",
      },
      disabled: {
        type: Boolean,
        default: false,
      },
      placeholder: {
        type: String,
        default: "",
      },
      type: {
        type: String,
        default: "text",
      },
    },
    emits: ["update:value"],
    setup(props, { emit }) {
      return () =>
        h("input", {
          value: props.value,
          disabled: props.disabled,
          placeholder: props.placeholder,
          type: props.type,
          onInput: (event: Event) => {
            const target = event.target as HTMLInputElement;
            emit("update:value", target.value);
          },
        });
    },
  }),
  NSelect: defineComponent({
    name: "NSelectStub",
    props: {
      value: {
        type: String,
        default: "",
      },
      options: {
        type: Array as PropType<Array<{ label: string; value: string }>>,
        default: () => [],
      },
      disabled: {
        type: Boolean,
        default: false,
      },
    },
    emits: ["update:value"],
    setup(props, { emit }) {
      return () =>
        h(
          "select",
          {
            value: props.value,
            disabled: props.disabled,
            onChange: (event: Event) => {
              const target = event.target as HTMLSelectElement;
              emit("update:value", target.value);
            },
          },
          props.options.map((option) =>
            h("option", { key: option.value, value: option.value }, option.label),
          ),
        );
    },
  }),
  NSwitch: defineComponent({
    name: "NSwitchStub",
    props: {
      value: { type: Boolean, default: false },
      disabled: { type: Boolean, default: false },
    },
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
  }),
};

export function mountWithPinia(
  component: Component,
  options: MountWithPiniaOptions = {},
): { wrapper: VueWrapper<any>; pinia: Pinia } {
  const pinia = options.pinia ?? createPinia();
  setActivePinia(pinia);

  const wrapper = mount(component, {
    ...options,
    global: {
      stubs: {
        ...naiveUiStubs,
        ...(options.global?.stubs ?? {}),
      },
      plugins: [pinia, ...(options.global?.plugins ?? [])],
      provide: options.global?.provide,
      config: options.global?.config,
      directives: options.global?.directives,
      mocks: options.global?.mocks,
      components: options.global?.components,
      mixins: options.global?.mixins,
      renderStubDefaultSlot: options.global?.renderStubDefaultSlot,
    },
  });

  return { wrapper, pinia };
}

export function flushPromises() {
  return flushVuePromises();
}

export function createEventSourceMock() {
  const instances: MockEventSourceInstance[] = [];

  class MockEventSourceInstance {
    url: string;
    withCredentials = false;
    readyState = 0;
    listeners = new Map<string, Set<Listener>>();
    close = vi.fn(() => {
      this.readyState = 2;
    });
    addEventListener = vi.fn((type: string, listener: Listener) => {
      const listeners = this.listeners.get(type) ?? new Set<Listener>();
      listeners.add(listener);
      this.listeners.set(type, listeners);
    });
    removeEventListener = vi.fn((type: string, listener: Listener) => {
      this.listeners.get(type)?.delete(listener);
    });

    constructor(url: string | URL) {
      this.url = String(url);
    }

    emit(type: string, event: Event) {
      for (const listener of this.listeners.get(type) ?? []) {
        listener(event);
      }
    }
  }

  class EventSourceMock extends MockEventSourceInstance {
    static calls: Array<string | URL> = [];

    constructor(url: string | URL) {
      super(url);
      EventSourceMock.calls.push(url);
      instances.push(this);
    }
  }

  return {
    EventSourceMock,
    instances,
  };
}

function slotStub(name: string): DefineComponent {
  return defineComponent({
    name: `${name}-stub`,
    setup(_, { slots }) {
      return () => h("div", { "data-test": name }, normalizeSlot(slots.default?.()));
    },
  });
}

function normalizeSlot(slot: VNode[] | undefined) {
  return slot ?? [];
}
