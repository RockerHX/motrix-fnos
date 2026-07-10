import { describe, expect, it, vi } from "vitest";
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
    NButton: defineComponent({
      name: "NButtonStub",
      props: {
        disabled: {
          type: Boolean,
          default: false,
        },
      },
      emits: ["click"],
      setup(props, { emit, slots, attrs }) {
        return () =>
          h(
            "button",
            {
              ...attrs,
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
    NCard: defineComponent({
      name: "NCardStub",
      setup(_, { slots }) {
        return () =>
          h("div", { "data-test": "n-card" }, [
            ...(slots.default?.() ?? []),
            ...(slots.footer?.() ?? []),
          ]);
      },
    }),
    NDescriptions: slotStub("n-descriptions"),
    NDescriptionsItem: defineComponent({
      name: "NDescriptionsItemStub",
      props: {
        label: {
          type: String,
          required: true,
        },
      },
      setup(props, { slots }) {
        return () => h("div", { "data-test": "n-descriptions-item" }, [props.label, slots.default?.()]);
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
    NSpace: slotStub("n-space"),
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
  };
});

import TaskActions from "./TaskActions.vue";
import { flushPromises, mountWithPinia } from "../../../test/mount";
import type {
  TaskActionConfirmTexts,
  TaskActionDetails,
  TaskActionLabels,
  TaskActionPermissions,
  TaskActionState,
} from "./taskActionViewModel";

describe("TaskActions", () => {
  it("shows buttons for active, paused, complete and removed states", async () => {
    const { wrapper: activeWrapper } = mountTaskActions({
      permissions: { canPause: true },
    });
    expect(activeWrapper.text()).toContain("暂停");
    expect(activeWrapper.text()).not.toContain("继续");

    const { wrapper: pausedWrapper } = mountTaskActions({
      permissions: { canResume: true },
    });
    expect(pausedWrapper.text()).toContain("继续");
    expect(pausedWrapper.text()).not.toContain("暂停");

    const { wrapper: confirmWrapper } = mountTaskActions({
      permissions: {
        canConfirmFiles: true,
        canResume: false,
      },
    });
    expect(confirmWrapper.text()).toContain("确认文件");
    expect(confirmWrapper.text()).not.toContain("继续");

    const { wrapper: completeWrapper } = mountTaskActions({
      permissions: { canRedownload: true },
    });
    expect(completeWrapper.text()).toContain("重新下载");

    const { wrapper: removedWrapper } = mountTaskActions({
      permissions: {
        canPermanentDelete: true,
        canDelete: false,
      },
    });
    expect(removedWrapper.text()).toContain("永久删除");
  });

  it("emits pause, resume and confirmFiles in both normal and compact layouts", async () => {
    const { wrapper } = mountTaskActions({
      permissions: {
        canPause: true,
        canResume: true,
        canConfirmFiles: true,
      },
    });

    await clickButton(wrapper, "暂停");
    await clickButton(wrapper, "继续");
    await clickButton(wrapper, "确认文件");

    expect(wrapper.emitted("pause")).toHaveLength(1);
    expect(wrapper.emitted("resume")).toHaveLength(1);
    expect(wrapper.emitted("confirmFiles")).toHaveLength(1);

    const { wrapper: compactWrapper } = mountTaskActions({
      compact: true,
      permissions: {
        canPause: true,
        canResume: true,
        canConfirmFiles: true,
      },
    });

    await clickButton(compactWrapper, "暂停");
    await clickButton(compactWrapper, "继续");
    await clickButton(compactWrapper, "确认文件");

    expect(compactWrapper.emitted("pause")).toHaveLength(1);
    expect(compactWrapper.emitted("resume")).toHaveLength(1);
    expect(compactWrapper.emitted("confirmFiles")).toHaveLength(1);
  });

  it("emits confirmDelete with deleteFiles=true when checkbox is selected", async () => {
    const { wrapper } = mountTaskActions({
      permissions: { canDelete: true },
    });

    await clickButton(wrapper, "删除");
    const checkbox = wrapper.get('input[type="checkbox"]');
    await checkbox.setValue(true);
    await clickButton(wrapper, "删除", -1);

    expect(wrapper.emitted("confirmDelete")).toEqual([[true]]);
  });

  it("renders details from ordered detail items", async () => {
    const { wrapper } = mountTaskActions({
      details: {
        items: [
          { label: "任务名称", value: "file.iso" },
          { label: "状态", value: "下载中" },
        ],
      },
    });

    await clickButton(wrapper, "详情");

    const detailItems = wrapper.findAll('[data-test="n-descriptions-item"]');
    expect(detailItems.map((item) => item.text())).toEqual(["任务名称file.iso", "状态下载中"]);
  });

  it("closes opened modals when runtime starts exiting", async () => {
    const { wrapper, props } = mountTaskActions({
      permissions: { canDelete: true },
      state: { isRuntimeExiting: false },
    });

    await clickButton(wrapper, "删除");
    expect(wrapper.findAll('[data-test="n-modal"]')).toHaveLength(1);

    await wrapper.setProps({
      state: {
        ...props.state,
        isRuntimeExiting: true,
      },
    });
    await flushPromises();

    expect(wrapper.findAll('[data-test="n-modal"]')).toHaveLength(0);
  });

  it("renders only permitted icon-pill actions with accessible labels and titles", () => {
    const { wrapper } = mountTaskActions({
      variant: "icon-pill",
      permissions: {
        canPause: true,
        canResume: false,
        canConfirmFiles: true,
        canRedownload: false,
        canDelete: false,
        canPermanentDelete: false,
      },
    });

    expect(findIconButton(wrapper, "详情").exists()).toBe(true);
    expect(findIconButton(wrapper, "暂停").exists()).toBe(true);
    expect(findIconButton(wrapper, "确认文件").exists()).toBe(true);
    expect(findIconButton(wrapper, "继续").exists()).toBe(false);
    expect(findIconButton(wrapper, "重新下载").exists()).toBe(false);
    expect(findIconButton(wrapper, "删除").exists()).toBe(false);
    expect(findIconButton(wrapper, "永久删除").exists()).toBe(false);

    for (const label of ["详情", "暂停", "确认文件"]) {
      const button = findIconButton(wrapper, label);
      expect(button.attributes("aria-label")).toBe(label);
      expect(button.attributes("title")).toBe(label);
    }
  });

  it("emits direct actions from icon-pill buttons", async () => {
    const { wrapper } = mountTaskActions({
      variant: "icon-pill",
      permissions: {
        canPause: true,
        canResume: true,
        canConfirmFiles: true,
      },
    });

    await clickIconButton(wrapper, "暂停");
    await clickIconButton(wrapper, "继续");
    await clickIconButton(wrapper, "确认文件");

    expect(wrapper.emitted("pause")).toHaveLength(1);
    expect(wrapper.emitted("resume")).toHaveLength(1);
    expect(wrapper.emitted("confirmFiles")).toHaveLength(1);
  });

  it("opens existing confirmation modals from icon-pill buttons", async () => {
    const { wrapper: redownloadWrapper } = mountTaskActions({
      variant: "icon-pill",
      permissions: { canRedownload: true, canDelete: false },
    });
    await clickIconButton(redownloadWrapper, "重新下载");
    expect(redownloadWrapper.find('[data-test="n-modal"]').text()).toContain("确认重新下载");

    const { wrapper: deleteWrapper } = mountTaskActions({
      variant: "icon-pill",
      permissions: { canDelete: true },
    });
    await clickIconButton(deleteWrapper, "删除");
    expect(deleteWrapper.find('[data-test="n-modal"]').text()).toContain("确认删除");

    const { wrapper: permanentDeleteWrapper } = mountTaskActions({
      variant: "icon-pill",
      permissions: { canDelete: false, canPermanentDelete: true },
    });
    await clickIconButton(permanentDeleteWrapper, "永久删除");
    expect(permanentDeleteWrapper.find('[data-test="n-modal"]').text()).toContain("确认永久删除");
  });
});

interface MountTaskActionsOverrides {
  compact?: boolean;
  variant?: "text" | "icon-pill";
  state?: Partial<TaskActionState>;
  permissions?: Partial<TaskActionPermissions>;
  labels?: Partial<TaskActionLabels>;
  details?: Partial<TaskActionDetails>;
  confirmTexts?: Partial<TaskActionConfirmTexts>;
}

function mountTaskActions(overrides: MountTaskActionsOverrides = {}) {
  const props = {
    compact: overrides.compact ?? false,
    variant: overrides.variant ?? "text",
    state: {
      isOperating: false,
      isActionDisabled: false,
      isRuntimeExiting: false,
      ...overrides.state,
    },
    permissions: {
      canPause: false,
      canResume: false,
      canConfirmFiles: false,
      canRedownload: false,
      canDelete: true,
      canPermanentDelete: false,
      ...overrides.permissions,
    },
    labels: {
      details: "详情",
      pause: "暂停",
      resume: "继续",
      confirmFiles: "确认文件",
      redownload: "重新下载",
      delete: "删除",
      permanentDelete: "永久删除",
      cancel: "取消",
      close: "关闭",
      ...overrides.labels,
    },
    details: {
      title: "任务详情",
      items: [
        { label: "任务名称", value: "file.iso" },
        { label: "状态", value: "下载中" },
        { label: "进度", value: "20%" },
        { label: "大小", value: "20 MB / 100 MB" },
        { label: "速度", value: "1 MB/s" },
        { label: "保存路径", value: "/downloads" },
        { label: "文件路径", value: "/downloads/file.iso" },
        { label: "GID", value: "gid-1" },
        { label: "链接", value: "https://example.com/file.iso" },
        { label: "创建时间", value: "2026-07-06 10:00" },
        { label: "更新时间", value: "2026-07-06 10:01" },
        { label: "错误原因", value: "网络断开" },
      ],
      ...overrides.details,
    },
    confirmTexts: {
      redownloadTitle: "重新下载",
      redownloadConfirmText: "确认重新下载",
      deleteTitle: "删除任务",
      deleteConfirmText: "确认删除",
      deleteFilesLabel: "同时删除本地文件",
      permanentDeleteTitle: "永久删除",
      permanentDeleteConfirmText: "确认永久删除",
      ...overrides.confirmTexts,
    },
  };

  const mounted = mountWithPinia(TaskActions, { props });

  return {
    props,
    ...mounted,
  };
}

async function clickButton(wrapper: ReturnType<typeof mountTaskActions>["wrapper"], text: string, index = 0) {
  const matches = wrapper.findAll("button").filter((item) => item.text() === text);
  const normalizedIndex = index >= 0 ? index : matches.length + index;
  const button = matches[normalizedIndex];

  expect(button, `button ${text} at index ${index} should exist`).toBeTruthy();
  await button!.trigger("click");
  await flushPromises();
}

function findIconButton(wrapper: ReturnType<typeof mountTaskActions>["wrapper"], label: string) {
  return wrapper.find(`button[aria-label="${label}"]`);
}

async function clickIconButton(wrapper: ReturnType<typeof mountTaskActions>["wrapper"], label: string) {
  const button = findIconButton(wrapper, label);
  expect(button.exists(), `icon button ${label} should exist`).toBe(true);
  await button.trigger("click");
  await flushPromises();
}
