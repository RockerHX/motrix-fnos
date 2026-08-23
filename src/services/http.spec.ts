import { afterEach, describe, expect, it, vi } from "vitest";
import {
  httpDelete,
  httpGet,
  httpGetBlob,
  httpPost,
  httpPostFormData,
  httpPut,
  setCsrfTokenProvider,
  setUnauthorizedHandler,
} from "./http";

describe("http client", () => {
  afterEach(() => {
    window.history.replaceState({}, "", "/");
    vi.unstubAllGlobals();
    setCsrfTokenProvider(null);
    setUnauthorizedHandler(null);
  });

  it("serializes JSON requests with a same-origin API path", async () => {
    const fetchMock = vi.fn().mockImplementation(() => Promise.resolve(jsonResponse({ ok: true })));
    vi.stubGlobal("fetch", fetchMock);

    await expect(httpPost<{ ok: boolean }>("/api/tasks", { url: "https://example.com/a.iso" })).resolves.toEqual({
      ok: true,
    });

    expect(fetchMock).toHaveBeenCalledWith("/api/tasks", {
      method: "POST",
      credentials: "same-origin",
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
      credentials: "same-origin",
      headers: {},
      body: formData,
    });
  });

  it("forwards AbortSignal without changing requests that do not need cancellation", async () => {
    const fetchMock = vi.fn().mockResolvedValue(jsonResponse([]));
    vi.stubGlobal("fetch", fetchMock);
    const controller = new AbortController();

    await httpGet("/api/tasks", { signal: controller.signal });

    expect(fetchMock).toHaveBeenCalledWith("/api/tasks", {
      method: "GET",
      credentials: "same-origin",
      headers: {},
      body: undefined,
      signal: controller.signal,
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

  it("returns authenticated binary downloads as Blob values", async () => {
    const fetchMock = vi.fn().mockResolvedValue(
      new Response(new Uint8Array([80, 75, 3, 4]), {
        status: 200,
        headers: { "content-type": "application/zip" },
      }),
    );
    vi.stubGlobal("fetch", fetchMock);

    const blob = await httpGetBlob("/api/diagnostics/diagnostic-bundle");

    expect(blob.type).toBe("application/zip");
    await expect(blob.arrayBuffer()).resolves.toEqual(new Uint8Array([80, 75, 3, 4]).buffer);
    expect(fetchMock).toHaveBeenCalledWith("/api/diagnostics/diagnostic-bundle", {
      method: "GET",
      credentials: "same-origin",
      headers: {},
      body: undefined,
    });
  });

  it("throws structured API errors", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue(
        jsonResponse({ code: "authentication_required", message: "管理会话已过期", reason: "session_expired" }, 401),
      ),
    );

    const promise = httpGet("/api/storage/accessible-paths");

    await expect(promise).rejects.toMatchObject({
      name: "ApiError",
      code: "authentication_required",
      status: 401,
      message: "管理会话已过期",
      reason: "session_expired",
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

  it("adds an in-memory csrf token only to write requests", async () => {
    const fetchMock = vi.fn().mockImplementation(() => Promise.resolve(jsonResponse({ ok: true })));
    vi.stubGlobal("fetch", fetchMock);
    setCsrfTokenProvider(() => "csrf-value");

    await httpGet("/api/tasks");
    await httpPut("/api/settings", {});

    expect(fetchMock.mock.calls[0][1].headers).toEqual({});
    expect(fetchMock.mock.calls[1][1].headers).toEqual({
      "content-type": "application/json",
      "X-CSRF-Token": "csrf-value",
    });
  });

  it("handles concurrent business 401 responses once and allows public auth requests to opt out", async () => {
    const unauthorized = vi.fn();
    setUnauthorizedHandler(unauthorized);
    vi.stubGlobal(
      "fetch",
      vi
        .fn()
        .mockImplementation(() =>
          Promise.resolve(jsonResponse({ code: "authentication_required", message: "请先登录" }, 401)),
        ),
    );

    const results = await Promise.allSettled([httpGet("/api/tasks"), httpGet("/api/settings")]);
    expect(results.every((result) => result.status === "rejected")).toBe(true);
    expect(unauthorized).toHaveBeenCalledOnce();

    await expect(httpPost("/api/auth/login", { password: "wrong" }, { handleUnauthorized: false })).rejects.toMatchObject({
      status: 401,
    });
    expect(unauthorized).toHaveBeenCalledOnce();
  });

  it("keeps unauthorized handling for binary downloads", async () => {
    const unauthorized = vi.fn();
    setUnauthorizedHandler(unauthorized);
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue(jsonResponse({ code: "authentication_required", message: "请先登录" }, 401)),
    );

    await expect(httpGetBlob("/api/diagnostics/diagnostic-bundle")).rejects.toMatchObject({
      code: "authentication_required",
      status: 401,
    });
    expect(unauthorized).toHaveBeenCalledOnce();
  });
});

function jsonResponse(payload: unknown, status = 200) {
  return new Response(JSON.stringify(payload), {
    status,
    headers: { "content-type": "application/json; charset=utf-8" },
  });
}
