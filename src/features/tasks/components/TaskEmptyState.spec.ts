import { describe, expect, it } from "vitest";
import TaskEmptyState from "./TaskEmptyState.vue";
import { flushPromises, mountWithPinia } from "../../../test/mount";

describe("TaskEmptyState", () => {
  it("renders default title and description", () => {
    const { wrapper } = mountWithPinia(TaskEmptyState);

    expect(wrapper.text()).toContain("暂无任务");
    expect(wrapper.text()).toContain("点击下方按钮或粘贴 HTTP / HTTPS 链接开始您的第一次下载。");
  });

  it("does not emit create when create action is disabled", async () => {
    const { wrapper } = mountWithPinia(TaskEmptyState, {
      props: {
        disableCreateAction: true,
      },
    });

    await clickButton(wrapper, "添加任务");

    expect(wrapper.emitted("create")).toBeUndefined();
  });

  it("emits openSettings from settings action", async () => {
    const { wrapper } = mountWithPinia(TaskEmptyState);

    await clickButton(wrapper, "打开设置");

    expect(wrapper.emitted("openSettings")).toHaveLength(1);
  });

  it("controls create and settings action visibility", () => {
    const { wrapper } = mountWithPinia(TaskEmptyState, {
      props: {
        showCreateAction: false,
        showSettingsAction: false,
      },
    });

    expect(findButton(wrapper, "添加任务")).toBeUndefined();
    expect(findButton(wrapper, "打开设置")).toBeUndefined();
  });
});

async function clickButton(wrapper: ReturnType<typeof mountWithPinia>["wrapper"], text: string) {
  const button = findButton(wrapper, text);
  expect(button, `button ${text} should exist`).toBeTruthy();
  await button!.trigger("click");
  await flushPromises();
}

function findButton(wrapper: ReturnType<typeof mountWithPinia>["wrapper"], text: string) {
  return wrapper.findAll("button").find((item) => item.text() === text);
}
