import { expect, test, type Page, type Route } from "@playwright/test";

const password = "test-password-123";
const accessToken = "e2e-access-token";
const task = {
  id: 1,
  url: "https://example.test/file.iso",
  sourceType: "url",
  fileName: "file.iso",
  saveDir: "/downloads",
  ownedTaskDir: null,
  category: "",
  gid: "e2e-gid",
  status: "paused",
  totalLength: 1024,
  completedLength: 0,
  downloadSpeed: 0,
  errorCode: null,
  errorMessage: null,
  filePath: null,
  useProxy: false,
  confirmationRequired: false,
  files: [],
  createdAt: 1,
  updatedAt: 1,
};

test.describe("browser smoke", () => {
  test("completes setup, opens task form, creates a URL task, and receives SSE snapshot", async ({ page }) => {
    const fixture = await installFixture(page);

    await page.goto("/");
    await expect(page.locator('[data-test="auth-password"]')).toBeVisible();
    await page.locator('[data-test="auth-bootstrap-token"] input').fill("local-bootstrap-token");
    await page.locator('[data-test="auth-password"] input').fill(password);
    await page.locator('[data-test="auth-password-confirm"] input').fill(password);
    await page.locator('[data-test="auth-submit"]').click();

    await expect(page.locator(".window-shell")).toBeVisible();
    await expect.poll(() => fixture.eventRequests).toBeGreaterThan(0);
    await expect(page.getByText("暂无任务")).toBeVisible();

    await page.locator('button[aria-label="新建任务"]:visible, button[aria-label="添加任务"]:visible').first().click();
    await expect(page.getByRole("dialog")).toBeVisible();
    const dialog = page.getByRole("dialog");
    await dialog.locator("textarea").fill("https://example.test/file.iso");
    await dialog.locator(".n-base-selection").click();
    await page.getByText("/downloads", { exact: true }).last().click({ force: true });
    await dialog.getByRole("button", { name: "开始下载" }).click();

    await expect.poll(() => fixture.createdUrls).toEqual(["https://example.test/file.iso"]);
    await expect(page.getByText("file.iso", { exact: true })).toBeVisible();
  });

  test("handles an expired API token by returning to the login gate", async ({ page }) => {
    const fixture = await installFixture(page);
    await completeSetup(page);
    fixture.forceUnauthorized = true;

    await page.reload();
    await expect(page.locator('[data-test="auth-password"]')).toBeVisible();
    await expect(page.locator('[data-test="auth-submit"]')).toBeVisible();
  });

  test("reconnects the SSE stream after the first connection closes", async ({ page }) => {
    const fixture = await installFixture(page);
    await completeSetup(page);

    await expect.poll(() => fixture.eventRequests, { timeout: 6_000 }).toBeGreaterThan(1);
  });

  test("renders the task shell at a mobile viewport", async ({ page }) => {
    await installFixture(page);
    await completeSetup(page);

    await expect(page.locator(".window-shell")).toBeVisible();
    if ((page.viewportSize()?.width ?? 1024) < 768) {
      await expect(page.locator(".mobile-actions")).toBeVisible();
    } else {
      await expect(page.locator(".desktop-actions")).toBeVisible();
    }
  });
});

async function completeSetup(page: Page) {
  await page.goto("/");
  await expect(page.locator('[data-test="auth-password"]')).toBeVisible();
  await page.locator('[data-test="auth-bootstrap-token"] input').fill("local-bootstrap-token");
  await page.locator('[data-test="auth-password"] input').fill(password);
  await page.locator('[data-test="auth-password-confirm"] input').fill(password);
  await page.locator('[data-test="auth-submit"]').click();
  await expect(page.locator(".window-shell")).toBeVisible();
}

async function installFixture(page: Page) {
  const fixture = {
    setupComplete: false,
    forceUnauthorized: false,
    eventRequests: 0,
    createdUrls: [] as string[],
  };

  await page.route("**/api/**", async (route) => {
    await handleApiRoute(route, fixture);
  });
  return fixture;
}

async function handleApiRoute(
  route: Route,
  fixture: { setupComplete: boolean; forceUnauthorized: boolean; eventRequests: number; createdUrls: string[] },
) {
  const request = route.request();
  const url = new URL(request.url());
  const path = url.pathname;

  if (path === "/api/events") {
    fixture.eventRequests += 1;
    await route.fulfill({
      status: 200,
      headers: { "content-type": "text/event-stream", "cache-control": "no-cache" },
      body: `event: tasks.snapshot\ndata: ${JSON.stringify({ revision: fixture.eventRequests, tasks: [] })}\n\n`,
    });
    return;
  }

  if (fixture.forceUnauthorized && request.method() !== "GET" && path !== "/api/auth/status") {
    await json(route, 401, { code: "jwt_invalid", message: "登录已失效" });
    return;
  }

  if (path === "/api/auth/status") {
    await json(
      route,
      200,
      fixture.forceUnauthorized
        ? { setupRequired: false, authenticated: false }
        : fixture.setupComplete
          ? { setupRequired: false, authenticated: true }
          : { setupRequired: true, authenticated: false },
    );
    return;
  }
  if (path === "/api/auth/setup") {
    fixture.setupComplete = true;
    await json(route, 200, { setupRequired: false, authenticated: true, accessToken });
    return;
  }
  if (path === "/api/auth/login") {
    fixture.setupComplete = true;
    await json(route, 200, { setupRequired: false, authenticated: true, accessToken });
    return;
  }
  if (path === "/api/app/info") {
    await json(route, 200, { name: "Motrix", version: "1.9.8", backendStatus: "ready", maintainer: "test", repositoryUrl: "https://example.test", releasePageUrl: "https://example.test/releases", targetArch: "x86_64", updateMode: "manual_fpk_or_app_center" });
    return;
  }
  if (path === "/api/app/ping") {
    await json(route, 200, { ok: true, message: "Rust 后端通信正常" });
    return;
  }
  if (path === "/api/tasks" && request.method() === "POST") {
    const payload = request.postDataJSON() as { url?: string };
    const urlValue = payload.url ?? "";
    fixture.createdUrls.push(urlValue);
    await json(route, 200, { ...task, url: urlValue, fileName: "file.iso" });
    return;
  }
  if (path === "/api/tasks/batch" && request.method() === "POST") {
    const payload = request.postDataJSON() as { urls?: string[] };
    const urls = payload.urls ?? [];
    fixture.createdUrls.push(...urls);
    await json(route, 200, {
      created: urls.map((urlValue) => ({ ...task, url: urlValue, fileName: "file.iso" })),
      failed: [],
    });
    return;
  }
  if (path === "/api/tasks") {
    if (fixture.forceUnauthorized) {
      await json(route, 401, { code: "jwt_invalid", message: "登录已失效" });
    } else {
      await json(route, 200, fixture.createdUrls.length ? [{ ...task, url: fixture.createdUrls[0] }] : []);
    }
    return;
  }
  if (path === "/api/tasks" && url.searchParams.get("status") === "removed") {
    await json(route, 200, []);
    return;
  }
  if (path === "/api/settings") {
    await json(route, 200, { defaultDownloadDir: "/downloads", maxConcurrentDownloads: 5, maxConnectionPerServer: 1, split: 5, minSplitSize: "20M", connectTimeout: 60, maxTries: 5, downloadLimit: 0, uploadLimit: 0, language: "zh-CN" });
    return;
  }
  if (path === "/api/storage/accessible-paths") {
    await json(route, 200, { paths: ["/downloads"] });
    return;
  }
  if (path === "/api/storage/display-paths") {
    await json(route, 200, { paths: [{ path: "/downloads", displayPath: "/downloads" }] });
    return;
  }
  if (path === "/api/aria2/process") {
    await json(route, 200, { running: false, pid: null, binarySource: "sidecar", message: "已停止" });
    return;
  }
  if (path === "/api/aria2/rpc") {
    await json(route, 200, { connected: false, version: null, message: "Aria2 未运行" });
    return;
  }
  if (path === "/api/aria2/config") {
    await json(route, 200, { configured: true, aria2Path: null, binarySource: "sidecar", message: "" });
    return;
  }
  if (path.startsWith("/api/")) {
    await json(route, 200, {});
    return;
  }
  await route.continue();
}

async function json(route: Route, status: number, body: unknown) {
  await route.fulfill({
    status,
    contentType: "application/json",
    body: JSON.stringify(body),
  });
}
