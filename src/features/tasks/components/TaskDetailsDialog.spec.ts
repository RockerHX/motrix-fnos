import { defineComponent, h } from "vue";
import { describe, expect, it, vi } from "vitest";
import { mount } from "@vue/test-utils";
import { naiveUiStubs } from "../../../test/mount";

vi.mock("naive-ui", () => ({
  ...naiveUiStubs,
  NCard: defineComponent({
    setup(_, { slots }) {
      return () => h("div", [slots.default?.(), slots.footer?.()]);
    },
  }),
  NDescriptionsItem: defineComponent({
    props: { label: String },
    setup(props, { slots }) {
      return () => h("div", { "data-test": "detail-item" }, [props.label, slots.default?.()]);
    },
  }),
}));

import TaskDetailsDialog from "./TaskDetailsDialog.vue";

describe("TaskDetailsDialog", () => {
  it("renders ordered details and emits close", async () => {
    const wrapper = mount(TaskDetailsDialog, {
      props: {
        show: true,
        closeLabel: "关闭",
        details: {
          title: "任务详情",
          items: [
            { label: "任务名称", value: "file.iso" },
            { label: "状态", value: "下载中" },
          ],
        },
      },
    });

    expect(wrapper.findAll('[data-test="detail-item"]').map((item) => item.text())).toEqual([
      "任务名称file.iso",
      "状态下载中",
    ]);
    await wrapper.get("button").trigger("click");
    expect(wrapper.emitted("update:show")).toEqual([[false]]);
  });
});
