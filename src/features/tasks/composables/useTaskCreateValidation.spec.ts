import { reactive, ref } from "vue";
import { describe, expect, it } from "vitest";
import { createTaskCreateFormState, type TaskCreateInputType } from "./taskCreateFormModel";
import { useTaskCreateValidation } from "./useTaskCreateValidation";

describe("useTaskCreateValidation", () => {
  it("parses URL lines and reports invalid line numbers", () => {
    const { form, validation } = setup();
    form.urls = "https://example.com/a.iso\nftp://example.com/b.iso\nhttps://example.com/c.iso";

    expect(validation.urlList.value).toHaveLength(3);
    expect(validation.invalidUrlLines.value).toEqual([2]);
    expect(validation.urlFeedback.value).toBe("第 2 行不是有效的 HTTP / HTTPS 链接。");
    expect(validation.validationError()).toBe("请输入有效的 HTTP / HTTPS 下载链接，并修正无效行");
  });

  it("validates URL, torrent and magnet sources", () => {
    const { form, activeInputType, validation } = setup();
    form.saveDir = "/downloads";
    form.urls = "https://example.com/file.iso";
    expect(validation.validationError()).toBeNull();

    activeInputType.value = "torrent";
    expect(validation.validationError()).toBe("请选择 .torrent 种子文件");
    form.torrentFile = new File(["torrent"], "example.torrent");
    expect(validation.validationError()).toBeNull();

    activeInputType.value = "magnet";
    form.magnet = "invalid";
    expect(validation.validationError()).toBe("请输入有效的磁力链接");
    form.magnet = "magnet:?xt=urn:btih:test";
    expect(validation.validationError()).toBeNull();
  });

  it("enforces connection and download limit boundaries", () => {
    const { form, validation } = setup();
    form.urls = "https://example.com/file.iso";
    form.saveDir = "/downloads";
    form.connections = 0;
    expect(validation.validationError()).toBe("请检查高级设置：连接数需为 1–64，限速不能小于 0");
    form.connections = 64;
    form.downloadLimitKb = -1;
    expect(validation.validationError()).toBe("请检查高级设置：连接数需为 1–64，限速不能小于 0");
    form.downloadLimitKb = 0;
    expect(validation.validationError()).toBeNull();
  });
});

function setup() {
  const form = reactive(createTaskCreateFormState());
  const activeInputType = ref<TaskCreateInputType>("url");
  return { form, activeInputType, validation: useTaskCreateValidation(form, activeInputType) };
}
