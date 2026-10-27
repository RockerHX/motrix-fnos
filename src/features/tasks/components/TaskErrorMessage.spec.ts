import { describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import TaskErrorMessage from "./TaskErrorMessage.vue";
import { createTask } from "../../../test/tasks";

describe("TaskErrorMessage", () => {
  it("renders formatted error with title for failed task", () => {
    const wrapper = mount(TaskErrorMessage, {
      props: {
        task: createTask({ status: "error", errorCode: "16", errorMessage: "network unreachable" }),
      },
    });

    const error = wrapper.get(".task-card-error");
    expect(wrapper.find('[data-test="task-card-error-slot"]').exists()).toBe(true);
    expect(error.classes()).toContain("task-card-error--single-line");
    expect(error.text()).toBe("错误码 16：network unreachable");
    expect(error.attributes("title")).toBe("错误码 16：network unreachable");
  });

  it("keeps slot container but hides text for non-error task", () => {
    const wrapper = mount(TaskErrorMessage, {
      props: {
        task: createTask(),
        variant: "multi-line",
      },
    });

    expect(wrapper.find('[data-test="task-card-error-slot"]').exists()).toBe(true);
    expect(wrapper.find(".task-card-error").exists()).toBe(false);
  });

  it("keeps long error text independently addressable", () => {
    const message = "连接被远端服务器重置，请检查网络、权限和目标地址后重试。".repeat(3);
    const wrapper = mount(TaskErrorMessage, {
      props: {
        task: createTask({ status: "error", errorMessage: message }),
        variant: "multi-line",
      },
    });

    const error = wrapper.get(".task-card-error");
    expect(error.classes()).toContain("task-card-error--multi-line");
    expect(error.attributes("title")).toBe(message);
    expect(error.text()).toBe(message);
  });
});
