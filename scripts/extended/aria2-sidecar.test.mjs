import assert from "node:assert/strict";
import { once } from "node:events";
import { createServer } from "node:http";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { spawn } from "node:child_process";
import { request } from "node:http";
import test from "node:test";

const requiresLinux = process.platform !== "linux";

test("sidecar starts, reports version, downloads, pauses, and resumes", { skip: requiresLinux }, async () => {
  const harness = await createHarness();
  try {
    const version = await harness.waitForRpc("aria2.getVersion");
    assert.equal(version.result?.version, "2.5.5");

    const download = await harness.rpc("aria2.addUri", [
      [harness.fixtureUrl("small.bin")],
      { dir: harness.directory, out: "small.bin", "file-allocation": "none" },
    ]);
    const gid = download.result;
    assert.match(gid, /^[a-f0-9]+$/);
    await waitForStatus(harness, gid, "complete");
    assert.equal((await readFile(path.join(harness.directory, "small.bin"))).length, 64 * 1024);

    const slow = await harness.rpc("aria2.addUri", [
      [harness.fixtureUrl("large.bin")],
      { dir: harness.directory, out: "large.bin", "file-allocation": "none", "max-download-limit": "1K" },
    ]);
    const slowGid = slow.result;
    await waitForStatus(harness, slowGid, "active");
    await harness.rpc("aria2.pause", [slowGid]);
    await waitForStatus(harness, slowGid, "paused");
    await harness.rpc("aria2.unpause", [slowGid]);
    await waitForStatus(harness, slowGid, "active");
  } finally {
    await harness.close();
  }
});

test("sidecar restores a paused download from its session file", { skip: requiresLinux }, async () => {
  const harness = await createHarness();
  try {
    const download = await harness.rpc("aria2.addUri", [
      [harness.fixtureUrl("large.bin")],
      { dir: harness.directory, out: "session.bin", "file-allocation": "none", "max-download-limit": "1K" },
    ]);
    const gid = download.result;
    await waitForStatus(harness, gid, "active");
    await harness.rpc("aria2.pause", [gid]);
    await waitForStatus(harness, gid, "paused");
    await harness.rpc("aria2.saveSession");
    await harness.stopSidecar();

    await harness.startSidecar();
    await harness.waitForRpc("aria2.getVersion");
    const status = await harness.rpc("aria2.tellStatus", [gid, ["status"]]);
    assert.ok(["paused", "waiting", "active"].includes(status.result?.status));
  } finally {
    await harness.close();
  }
});

test("sidecar RPC becomes unavailable after an unexpected process exit", { skip: requiresLinux }, async () => {
  const harness = await createHarness();
  try {
    await harness.waitForRpc("aria2.getVersion");
    harness.child.kill("SIGKILL");
    await once(harness.child, "exit");
    await assert.rejects(() => harness.rpc("aria2.getVersion"));
  } finally {
    await harness.close();
  }
});

test("sidecar startup failure reports engine output without waiting for the RPC timeout", { skip: requiresLinux, timeout: 5_000 }, async () => {
  await assert.rejects(
    () => createHarness(["--motrix-invalid-test-option=true"]),
    /sidecar exited:[\s\S]*motrix-invalid-test-option/,
  );
});

async function createHarness(extraArgs = []) {
  const directory = await mkdtemp(path.join(os.tmpdir(), "motrix-aria2-test-"));
  const session = path.join(directory, "aria2.session");
  await writeFile(session, "");
  const fixture = createFixtureServer();
  await fixture.listen();
  const harness = {
    directory,
    fixture,
    child: null,
    port: await getFreePort(),
    secret: "motrix-test-secret",
    fixtureUrl(name) {
      return `http://127.0.0.1:${fixture.port}/${name}`;
    },
    async startSidecar() {
      const binary = path.join(
        process.cwd(),
        "assets",
        "aria2",
        process.arch === "arm64" ? "aria2-next-aarch64-unknown-linux-gnu" : "aria2-next-x86_64-unknown-linux-gnu",
      );
      this.output = "";
      this.spawnError = null;
      this.child = spawn(binary, [
        "--no-conf=true",
        "--enable-rpc=true",
        "--rpc-listen-all=false",
        `--rpc-listen-port=${this.port}`,
        `--rpc-secret=${this.secret}`,
        `--dir=${directory}`,
        `--save-session=${session}`,
        `--input-file=${session}`,
        "--save-session-interval=1",
        "--file-allocation=none",
        "--console-log-level=warn",
        ...extraArgs,
      ], { stdio: ["ignore", "pipe", "pipe"] });
      const capture = (chunk) => { this.output = (this.output + chunk.toString()).slice(-8_192); };
      this.child.stdout.on("data", capture);
      this.child.stderr.on("data", capture);
      this.child.on("error", (error) => { this.spawnError = error; capture(error.message); });
      this.exited = new Promise((resolve) => this.child.once("close", resolve));
    },
    async stopSidecar() {
      if (!this.child || this.spawnError || this.child.exitCode !== null || this.child.signalCode !== null) return;
      this.child.kill("SIGTERM");
      await Promise.race([this.exited, delay(3_000)]);
      if (this.child.exitCode === null && this.child.signalCode === null) this.child.kill("SIGKILL");
      await Promise.race([this.exited, delay(2_000).then(() => { throw new Error("sidecar did not exit after SIGKILL"); })]);
    },
    async waitForRpc(method) {
      const deadline = Date.now() + 10_000;
      let lastError;
      while (Date.now() < deadline) {
        if (this.spawnError || this.child.exitCode !== null || this.child.signalCode !== null) {
          throw new Error(`sidecar exited: ${this.output}`, { cause: this.spawnError });
        }
        try {
          return await this.rpc(method);
        } catch (error) {
          lastError = error;
          await delay(100);
        }
      }
      throw new Error(`sidecar RPC did not become ready: ${this.output}`, { cause: lastError });
    },
    async rpc(method, params = []) {
      const payload = JSON.stringify({ jsonrpc: "2.0", id: Date.now(), method, params: [`token:${this.secret}`, ...params] });
      return new Promise((resolve, reject) => {
        const req = request(
          { hostname: "127.0.0.1", port: this.port, path: "/jsonrpc", method: "POST", headers: { "content-type": "application/json", "content-length": Buffer.byteLength(payload) } },
          (response) => {
            const chunks = [];
            response.on("data", (chunk) => chunks.push(chunk));
            response.on("end", () => {
              if ((response.statusCode ?? 500) >= 400) {
                reject(new Error(`Aria2 RPC HTTP ${response.statusCode}`));
                return;
              }
              try {
                const body = JSON.parse(Buffer.concat(chunks).toString("utf8"));
                if (body.error) reject(new Error(body.error.message));
                else resolve(body);
              } catch (error) {
                reject(error);
              }
            });
          },
        );
        req.setTimeout(3_000, () => req.destroy(new Error(`Aria2 RPC timed out: ${method}`)));
        req.on("error", reject);
        req.end(payload);
      });
    },
    async close() {
      await this.stopSidecar();
      await fixture.close();
      await rm(directory, { recursive: true, force: true });
    },
  };
  try {
    await harness.startSidecar();
    await harness.waitForRpc("aria2.getVersion");
  } catch (error) {
    await harness.close();
    throw error;
  }
  return harness;
}

async function waitForStatus(harness, gid, expected) {
  await retry(async () => {
    const response = await harness.rpc("aria2.tellStatus", [gid, ["status"]]);
    assert.equal(response.result?.status, expected);
  }, 15_000);
}

function createFixtureServer() {
  const server = createServer((request, response) => {
    if (request.url === "/small.bin") {
      response.writeHead(200, { "content-type": "application/octet-stream", "content-length": 64 * 1024 });
      response.end(Buffer.alloc(64 * 1024, 7));
      return;
    }
    if (request.url === "/large.bin") {
      response.writeHead(200, { "content-type": "application/octet-stream", "content-length": 256 * 1024 });
      response.end(Buffer.alloc(256 * 1024, 9));
      return;
    }
    response.writeHead(404).end();
  });
  return {
    port: 0,
    async listen() {
      await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
      this.port = server.address().port;
    },
    close() {
      return new Promise((resolve) => server.close(() => resolve()));
    },
  };
}

function getFreePort() {
  return new Promise((resolve, reject) => {
    const server = createServer();
    server.once("error", reject);
    server.listen(0, "127.0.0.1", () => {
      const port = server.address().port;
      server.close(() => resolve(port));
    });
  });
}

async function retry(operation, timeout = 10_000) {
  const deadline = Date.now() + timeout;
  let lastError;
  while (Date.now() < deadline) {
    try {
      return await operation();
    } catch (error) {
      lastError = error;
      await delay(100);
    }
  }
  throw lastError ?? new Error("operation timed out");
}

function delay(milliseconds) {
  return new Promise((resolve) => setTimeout(resolve, milliseconds));
}
