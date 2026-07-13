import { afterEach, describe, expect, it, vi } from "vitest";
import { httpDelete, httpGet, httpPost, httpPostFormData, httpPut } from "./http";

describe("http client", () => {
  afterEach(() => {
    window.history.replaceState({}, "", "/");
    vi.unstubAllGlobals();
  });

  it("serializes JSON requests and uses the fnOS gateway prefix", async () => {
    window.history.replaceState({}, "", "/app/motrix/");
    const fetchMock = vi.fn().mockResolvedValue(jsonResponse({ ok: true }));
    vi.stubGlobal("fetch", fetchMock);

    await expect(httpPost<{ ok: boolean }>("/api/tasks", { url: "https://example.com/a.iso" })).resolves.toEqual({
      ok: true,
    });

    expect(fetchMock).toHaveBeenCalledWith("/app/motrix/api/tasks", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ url: "https://example.com/a.iso" }),
    });
  });

  it("sends FormData without forcing a JSON content type", async () => {
    const fetchMock = vi.fn().mockResolvedValue(jsonResponse({ id: 1 }));
    vi.stubGlobal("fetch", fetchMock);
    const formData = new FormData();
    formData.append("torrent", new File(["torrent"], "movie.torrent"));

    await httpPostFormData("/api/tasks/torrent", formData);

    expect(fetchMock).toHaveBeenCalledWith("/api/tasks/torrent", {
      method: "POST",
      headers: {},
      body: formData,
    });
  });

  it("returns undefined for 204 responses", async () => {
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue(new Response(null, { status: 204 })));

    await expect(httpDelete<void>("/api/debug-logs")).resolves.toBeUndefined();
  });

  it("returns JSON and text success payloads", async () => {
    const fetchMock = vi
      .fn()
      .mockResolvedValueOnce(jsonResponse({ connected: true }))
      .mockResolvedValueOnce(new Response("ready", { status: 200, headers: { "content-type": "text/plain" } }));
    vi.stubGlobal("fetch", fetchMock);

    await expect(httpGet("/api/aria2/rpc")).resolves.toEqual({ connected: true });
    await expect(httpPut("/api/settings", {})).resolves.toBe("ready");
  });

  it("throws structured API errors", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue(jsonResponse({ code: "save_dir_unauthorized", message: "目录未授权" }, 400)),
    );

    const promise = httpGet("/api/storage/accessible-paths");

    await expect(promise).rejects.toMatchObject({
      name: "ApiError",
      code: "save_dir_unauthorized",
      status: 400,
      message: "目录未授权",
    });
  });

  it("falls back to text and status messages for non-standard errors", async () => {
    const fetchMock = vi
      .fn()
      .mockResolvedValueOnce(new Response("upstream unavailable", { status: 502 }))
      .mockResolvedValueOnce(new Response("", { status: 500 }));
    vi.stubGlobal("fetch", fetchMock);

    await expect(httpGet("/api/app/ping")).rejects.toMatchObject({
      code: "http_error",
      status: 502,
      message: "upstream unavailable",
    });
    await expect(httpGet("/api/app/ping")).rejects.toMatchObject({
      code: "http_error",
      status: 500,
      message: "请求失败（500）",
    });
  });
});

function jsonResponse(payload: unknown, status = 200) {
  return new Response(JSON.stringify(payload), {
    status,
    headers: { "content-type": "application/json; charset=utf-8" },
  });
}
