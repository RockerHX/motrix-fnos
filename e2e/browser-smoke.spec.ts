import { expect, test, type Page, type Route } from "@playwright/test";

const password = "test-password-123";
const accessToken = "e2e-access-token";
const task = {
  id: 1,
  engineGeneration: "native-v2",
  readOnly: false,
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
  filesDeleted: false,
  cleanupState: "none",
  metadataState: "none",
  confirmationRequired: false,
  files: [],
  createdAt: 1,
  updatedAt: 1,
};

test.describe("browser smoke", () => {
  test("shows the login form and keeps diagnostics visible", async ({ page }, testInfo) => {
    await installFixture(page);
    await page.goto("/");
    await expect(page.locator('[data-test="auth-password"]')).toBeVisible();
    await expect(page.locator('[data-test="auth-password-confirm"]')).toHaveCount(0);
    await expect(page.locator('[data-test="auth-bootstrap-token"]')).toHaveCount(0);
    await expect(page.locator('[data-test="auth-copy-diagnostic"]')).toBeVisible();
    await expect(page.locator('[data-test="auth-download-diagnostic"]')).toBeVisible();
    await expect(page.locator("#app-bootstrap")).toHaveCount(0);
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);
    await page.screenshot({ path: testInfo.outputPath("login.png"), fullPage: true });
  });

  test("keeps unconfigured installations protected and shows recovery diagnostics", async ({ page }, testInfo) => {
    await installFixture(page);
    await page.route("**/api/auth/status", (route) => json(route, 200, { setupRequired: true, authenticated: false }));
    await page.goto("/");
    await expect(page.locator('[data-test="auth-unconfigured"]')).toBeVisible();
    await expect(page.locator('[data-test="auth-password"]')).toHaveCount(0);
    await expect(page.locator(".window-shell")).toHaveCount(0);
    await expect(page.locator('[data-test="auth-download-diagnostic"]')).toBeVisible();
    await expect(page.locator("#app-bootstrap")).toHaveCount(0);
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);
    await page.screenshot({ path: testInfo.outputPath("unconfigured.png"), fullPage: true });
  });

  test("logs in, opens task form, creates a URL task, and receives SSE snapshot", async ({ page }) => {
    const fixture = await installFixture(page);

    await page.goto("/");
    await expect(page.locator('[data-test="auth-password"]')).toBeVisible();
    await page.locator('[data-test="auth-password"] input').fill(password);
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

  for (const deleteFiles of [false, true]) {
    test(`moves a task to Trash and permanently deletes it with deleteFiles=${deleteFiles}`, async ({ page }, testInfo) => {
      const fixture = await installFixture(page);
      fixture.createdUrls.push(task.url);
      fixture.eventTasks = [{ ...task }];
      let removed: (typeof task)[] = [];
      let cleanupDone = false;
      const deleteRequests: string[] = [];
      await page.route("**/api/tasks**", async (route) => {
        const url = new URL(route.request().url());
        if (url.pathname === "/api/tasks" && url.searchParams.get("status") === "removed") {
          await json(route, 200, cleanupDone ? [] : removed);
        } else if (url.pathname === "/api/tasks/1" && route.request().method() === "DELETE") {
          deleteRequests.push(url.pathname + url.search);
          fixture.createdUrls = [];
          fixture.eventTasks = [];
          removed = [{ ...task, status: "removed" }];
          await json(route, 200, removed[0]);
        } else if (url.pathname === "/api/tasks/1/permanent") {
          deleteRequests.push(url.pathname + url.search);
          if (url.searchParams.get("deleteFiles") === "true") {
            removed = [{ ...removed[0], cleanupState: "pending", filesDeleted: true }];
            await json(route, 202, removed[0]);
          } else {
            removed = [];
            await route.fulfill({ status: 204 });
          }
        } else {
          await route.fallback();
        }
      });
      await completeLogin(page);
      await page.getByRole("button", { name: "移入回收站", exact: true }).click();
      const dialog = page.getByRole("dialog");
      await expect(dialog.getByRole("checkbox")).toHaveCount(0);
      await expect(dialog).toContainText("此操作不会释放磁盘空间");
      await dialog.getByRole("button", { name: "移入回收站", exact: true }).click();
      await expect(page.getByText("file.iso", { exact: true })).toHaveCount(0);
      await page.getByRole("button", { name: "回收站", exact: true }).click();
      await page.getByRole("button", { name: "永久删除", exact: true }).click();
      const checkbox = dialog.getByRole("checkbox");
      await expect(checkbox).not.toBeChecked();
      if (deleteFiles) await checkbox.check();
      await page.screenshot({ path: testInfo.outputPath("permanent-delete.png"), fullPage: true });
      expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);
      await dialog.getByRole("button", { name: "永久删除", exact: true }).click();
      await expect(dialog).toHaveCount(0);
      if (deleteFiles) {
        await expect(page.getByText("文件清理中", { exact: true })).toBeVisible();
        cleanupDone = true;
      }
      await expect(page.getByText("file.iso", { exact: true })).toHaveCount(0);
      expect(deleteRequests).toEqual(["/api/tasks/1", `/api/tasks/1/permanent?deleteFiles=${deleteFiles}`]);
    });
  }

  test("handles an expired API token by returning to the login gate", async ({ page }) => {
    const fixture = await installFixture(page);
    await completeLogin(page);
    fixture.forceUnauthorized = true;

    await page.reload();
    await expect(page.locator('[data-test="auth-password"]')).toBeVisible();
    await expect(page.locator('[data-test="auth-submit"]')).toBeVisible();
  });

  test("reconnects the SSE stream after the first connection closes", async ({ page }) => {
    const fixture = await installFixture(page);
    await completeLogin(page);

    await expect.poll(() => fixture.eventRequests, { timeout: 6_000 }).toBeGreaterThan(1);
  });

  test("updates task progress from authenticated SSE snapshots without a page reload", async ({ page }) => {
    const fixture = await installFixture(page);
    fixture.eventTasks = [{ ...task, status: "active", completedLength: 256, downloadSpeed: 128 }];
    const eventRequest = page.waitForRequest((request) => new URL(request.url()).pathname === "/api/events");
    await completeLogin(page);

    expect((await eventRequest).headers().authorization).toBe(`Bearer ${accessToken}`);
    await expect(page.getByText("file.iso", { exact: true })).toBeVisible();
    const progress = page.getByRole("progressbar");
    await expect(progress).toHaveAttribute("aria-valuenow", "25");

    fixture.eventTasks = [{ ...fixture.eventTasks[0], completedLength: 768 }];
    await expect(progress).toHaveAttribute("aria-valuenow", "75", { timeout: 6_000 });
  });

  test("renders the task shell at a mobile viewport", async ({ page }) => {
    await installFixture(page);
    await completeLogin(page);

    await expect(page.locator(".window-shell")).toBeVisible();
    if ((page.viewportSize()?.width ?? 1024) < 768) {
      await expect(page.locator(".mobile-actions")).toBeVisible();
    } else {
      await expect(page.locator(".desktop-actions")).toBeVisible();
    }
  });

  test("opens task details with the keyboard and closes with Escape", async ({ page }) => {
    const fixture = await installFixture(page);
    fixture.createdUrls.push(task.url);
    fixture.eventTasks = [{ ...task }];
    await completeLogin(page);
    const details = page.getByRole("button", { name: "详情", exact: true });
    await expect(details).toHaveAttribute("aria-label", "详情");
    await details.focus();
    await details.press("Enter");
    await expect(page.getByRole("dialog")).toBeVisible();
    await page.keyboard.press("Escape");
    await expect(page.getByRole("dialog")).toHaveCount(0);
  });
});

async function completeLogin(page: Page) {
  await page.goto("/");
  await expect(page.locator('[data-test="auth-password"]')).toBeVisible();
  await page.locator('[data-test="auth-password"] input').fill(password);
  await page.locator('[data-test="auth-submit"]').click();
  await expect(page.locator(".window-shell")).toBeVisible({ timeout: 10_000 });
}

async function installFixture(page: Page) {
  const fixture = {
    authenticated: false,
    forceUnauthorized: false,
    eventRequests: 0,
    eventTasks: [] as (typeof task)[],
    createdUrls: [] as string[],
  };

  await page.route("**/api/**", async (route) => {
    await handleApiRoute(route, fixture);
  });
  return fixture;
}

async function handleApiRoute(
  route: Route,
  fixture: { authenticated: boolean; forceUnauthorized: boolean; eventRequests: number; eventTasks: (typeof task)[]; createdUrls: string[] },
) {
  const request = route.request();
  const url = new URL(request.url());
  const path = url.pathname;

  if (path === "/api/events") {
    fixture.eventRequests += 1;
    await route.fulfill({
      status: 200,
      headers: { "content-type": "text/event-stream", "cache-control": "no-cache" },
      body: `event: tasks.snapshot\ndata: ${JSON.stringify({ revision: fixture.eventRequests, tasks: fixture.eventTasks })}\n\n`,
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
        : fixture.authenticated
          ? { setupRequired: false, authenticated: true }
          : { setupRequired: false, authenticated: false },
    );
    return;
  }
  if (path === "/api/auth/login") {
    fixture.authenticated = true;
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
    await json(route, 200, { defaultDownloadDir: "/downloads", maxConcurrentDownloads: 5, httpMaxConnections: 1, connectTimeout: 60, maxTries: 5, downloadLimit: 0, uploadLimit: 0, language: "zh-CN" });
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
