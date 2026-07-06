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
    NDescriptionsItem: slotStub("n-descriptions-item"),
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

describe("TaskActions", () => {
  it("shows buttons for active, paused, complete and removed states", async () => {
    const { wrapper: activeWrapper } = mountTaskActions({
      canPause: true,
    });
    expect(activeWrapper.text()).toContain("暂停");
    expect(activeWrapper.text()).not.toContain("继续");

    const { wrapper: pausedWrapper } = mountTaskActions({
      canResume: true,
    });
    expect(pausedWrapper.text()).toContain("继续");
    expect(pausedWrapper.text()).not.toContain("暂停");

    const { wrapper: completeWrapper } = mountTaskActions({
      canRedownload: true,
    });
    expect(completeWrapper.text()).toContain("重新下载");

    const { wrapper: removedWrapper } = mountTaskActions({
      canPermanentDelete: true,
      canDelete: false,
    });
    expect(removedWrapper.text()).toContain("永久删除");
  });

  it("emits pause and resume in both normal and compact layouts", async () => {
    const { wrapper } = mountTaskActions({
      canPause: true,
      canResume: true,
    });

    await clickButton(wrapper, "暂停");
    await clickButton(wrapper, "继续");

    expect(wrapper.emitted("pause")).toHaveLength(1);
    expect(wrapper.emitted("resume")).toHaveLength(1);

    const { wrapper: compactWrapper } = mountTaskActions({
      compact: true,
      canPause: true,
      canResume: true,
    });

    await clickButton(compactWrapper, "暂停");
    await clickButton(compactWrapper, "继续");

    expect(compactWrapper.emitted("pause")).toHaveLength(1);
    expect(compactWrapper.emitted("resume")).toHaveLength(1);
  });

  it("emits confirmDelete with deleteFiles=true when checkbox is selected", async () => {
    const { wrapper } = mountTaskActions({
      canDelete: true,
    });

    await clickButton(wrapper, "删除");
    const checkbox = wrapper.get('input[type="checkbox"]');
    await checkbox.setValue(true);
    await clickButton(wrapper, "删除", -1);

    expect(wrapper.emitted("confirmDelete")).toEqual([[true]]);
  });

  it("closes opened modals when runtime starts exiting", async () => {
    const { wrapper } = mountTaskActions({
      canDelete: true,
      isRuntimeExiting: false,
    });

    await clickButton(wrapper, "删除");
    expect(wrapper.findAll('[data-test="n-modal"]')).toHaveLength(1);

    await wrapper.setProps({
      isRuntimeExiting: true,
    });
    await flushPromises();

    expect(wrapper.findAll('[data-test="n-modal"]')).toHaveLength(0);
  });
});

function mountTaskActions(overrides: Partial<InstanceType<typeof TaskActions>["$props"]> = {}) {
  return mountWithPinia(TaskActions, {
    props: {
      compact: false,
      isOperating: false,
      isActionDisabled: false,
      isRuntimeExiting: false,
      canPause: false,
      canResume: false,
      canRedownload: false,
      canDelete: true,
      canPermanentDelete: false,
      detailsLabel: "详情",
      pauseLabel: "暂停",
      resumeLabel: "继续",
      redownloadLabel: "重新下载",
      deleteLabel: "删除",
      permanentDeleteLabel: "永久删除",
      cancelLabel: "取消",
      closeLabel: "关闭",
      detailTitle: "任务详情",
      detailFileNameLabel: "任务名称",
      detailStatusLabel: "状态",
      detailProgressLabel: "进度",
      detailSizeLabel: "大小",
      detailSpeedLabel: "速度",
      detailSaveDirLabel: "保存路径",
      detailFilePathLabel: "文件路径",
      detailGidLabel: "GID",
      detailUrlLabel: "链接",
      detailCreatedAtLabel: "创建时间",
      detailUpdatedAtLabel: "更新时间",
      detailErrorReasonLabel: "错误原因",
      detailFileName: "file.iso",
      detailStatus: "下载中",
      detailProgress: "20%",
      detailSize: "20 MB / 100 MB",
      detailSpeed: "1 MB/s",
      detailSaveDir: "/downloads",
      detailFilePath: "/downloads/file.iso",
      detailGid: "gid-1",
      detailUrl: "https://example.com/file.iso",
      detailCreatedAt: "2026-07-06 10:00",
      detailUpdatedAt: "2026-07-06 10:01",
      detailErrorReason: "网络断开",
      redownloadTitle: "重新下载",
      redownloadConfirmText: "确认重新下载",
      deleteTitle: "删除任务",
      deleteConfirmText: "确认删除",
      deleteFilesLabel: "同时删除本地文件",
      permanentDeleteTitle: "永久删除",
      permanentDeleteConfirmText: "确认永久删除",
      ...overrides,
    },
  });
}

async function clickButton(wrapper: ReturnType<typeof mountTaskActions>["wrapper"], text: string, index = 0) {
  const matches = wrapper.findAll("button").filter((item) => item.text() === text);
  const normalizedIndex = index >= 0 ? index : matches.length + index;
  const button = matches[normalizedIndex];

  expect(button, `button ${text} at index ${index} should exist`).toBeTruthy();
  await button!.trigger("click");
  await flushPromises();
}
