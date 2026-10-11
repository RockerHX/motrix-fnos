import assert from "node:assert/strict";
import { once } from "node:events";
import { createServer } from "node:http";
import { chmod, mkdir, mkdtemp, readFile, readdir, rm, stat, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { execFile, spawn } from "node:child_process";
import { request } from "node:http";
import test from "node:test";
import { promisify } from "node:util";
import { sha256 } from "../lib/script-utils.mjs";
import { createHttpFixture, createPeerGateway, createProxyFixture, createTorrent, createUdpFixture, httpPayloads } from "./aria2-fixtures.mjs";

const requiresLinux = process.platform !== "linux";
const candidateDirectory = process.env.MOTRIX_ARIA2_CANDIDATE_DIR;
const candidateOnly = { skip: !candidateDirectory, timeout: 90_000 };
const candidates = {
  x64: { name: "aria2-next-2.8.6-linux-x86_64", hash: "51a053d0b4d63a33425ed757a4acbc2a5d479bbd811b82d9a68c7d7f7fe9020c" },
  arm64: { name: "aria2-next-2.8.6-linux-aarch64", hash: "c72d3bc670aa3ede1ba1964109397c388b3b6bd85ca5d269370b87652e9694ad" },
};
if (candidateDirectory && requiresLinux) throw new Error("290-02 candidate validation requires Linux; a skipped macOS run is not evidence");
let binaryPromise;

test("sidecar starts, reports version, downloads, pauses, and resumes", { skip: requiresLinux }, async () => {
  const harness = await createHarness();
  try {
    const version = await harness.waitForRpc("aria2.getVersion");
    assert.equal(version.result?.version, candidateDirectory ? "2.8.6" : "2.5.5");

    const download = await harness.rpc("aria2.addUri", [
      [harness.fixtureUrl("small.bin")],
      { dir: harness.directory, out: "small.bin", pause: "false", "file-allocation": "none" },
    ]);
    const gid = download.result;
    assert.match(gid, /^[a-f0-9]+$/);
    await waitForStatus(harness, gid, "complete");
    assert.equal(sha256(await readFile(path.join(harness.directory, "small.bin"))), sha256(httpPayloads["small.bin"]));

    const slow = await harness.rpc("aria2.addUri", [
      [harness.fixtureUrl("large.bin")],
      { dir: harness.directory, out: "large.bin", pause: "false", "file-allocation": "none", "max-download-limit": "1K" },
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
      { dir: harness.directory, out: "session.bin", pause: "false", "file-allocation": "none", "max-download-limit": "1K" },
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
    assert.equal(status.result?.status, "paused");
  } finally {
    await harness.close();
  }
});

test("sidecar RPC becomes unavailable after an unexpected process exit", { skip: requiresLinux }, async () => {
  const harness = await createHarness();
  try {
    await harness.waitForRpc("aria2.getVersion");
    const exited = once(harness.child, "exit");
    harness.child.kill("SIGKILL");
    await exited;
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

test("290-02 candidate has usable Linux symbols and writes as the service UID", candidateOnly, async (t) => {
  assert.notEqual(process.getuid(), 0, "run candidate tests as a non-root service user so permission checks are meaningful");
  const binary = await resolveBinary();
  const run = promisify(execFile);
  const libraries = await run("ldd", [binary]);
  assert.doesNotMatch(libraries.stdout, /not found/);
  const symbols = await run("readelf", ["--version-info", binary]);
  for (const symbol of ["GLIBC_2.35", "GLIBCXX_3.4.30", "CXXABI_1.3.13"]) assert.ok(symbols.stdout.includes(symbol), symbol);
  const harness = await createHarness();
  t.after(() => harness.close());
  await harness.waitForRpc("aria2.getVersion");
  const { result: gid } = await harness.rpc("aria2.addUri", [[harness.fixtureUrl("small.bin")], { out: "uid.bin", pause: "false", media: "file" }]);
  await waitForStatus(harness, gid, "complete");
  for (const file of [harness.stateDirectory, path.join(harness.directory, "uid.bin")]) {
    assert.equal((await stat(file)).uid, process.getuid());
  }
  const stateFiles = await readdir(harness.stateDirectory, { recursive: true });
  assert.ok(stateFiles.length > 0, "the candidate must persist native state inside state-dir");
  for (const file of stateFiles) assert.equal((await stat(path.join(harness.stateDirectory, file))).uid, process.getuid());
  t.diagnostic(JSON.stringify({ platform: process.platform, arch: process.arch, kernel: os.release(), uid: process.getuid(), binarySha256: candidates[process.arch].hash, libraries: libraries.stdout.trim() }));
});

test("290-02 HTTP retains GID, path and committed bytes across a paused restart; remove/readd creates a copy", candidateOnly, async (t) => {
  const harness = await createHarness();
  t.after(() => harness.close());
  const gid = await addPartialHttp(harness, "resume.bin");
  await harness.rpc("aria2.pause", [gid]);
  const paused = await waitForTask(harness, gid, (task) => task.status === "paused");
  const file = paused.files[0].path;
  const partialHash = sha256(await readFile(file));
  await saveAndRestart(harness);
  const restored = await waitForTask(harness, gid, (task) => task.status === "paused");
  assert.equal(restored.gid, gid);
  assert.equal(restored.files[0].path, file);
  assert.equal(restored.completedLength, paused.completedLength);
  assert.equal(sha256(await readFile(file)), partialHash);
  await assertStaysPaused(harness, gid);
  await harness.rpc("aria2.changeOption", [gid, { "max-download-limit": "0" }]);
  await harness.rpc("aria2.unpause", [gid]);
  await waitForStatus(harness, gid, "complete");
  assert.equal(sha256(await readFile(file)), sha256(httpPayloads["resume.bin"]));
  assert.ok(harness.fixture.requests.some((entry) => entry.range && Number(entry.range.match(/\d+/)[0]) > 0), "resume must request an existing byte range");

  const removedGid = await addPartialHttp(harness, "removed.bin");
  await harness.rpc("aria2.pause", [removedGid]);
  const removed = await waitForTask(harness, removedGid, (task) => task.status === "paused");
  const oldFile = removed.files[0].path;
  const oldHash = sha256(await readFile(oldFile));
  await harness.rpc("aria2.remove", [removedGid]);
  const { result: replacementGid } = await harness.rpc("aria2.addUri", [[harness.fixtureUrl("resume.bin")], {
    out: "removed.bin", pause: "false", media: "file", continue: "true", "stream-max-connections": "1",
  }]);
  const replacement = await waitForTask(harness, replacementGid, (task) => task.status === "complete");
  assert.notEqual(replacementGid, removedGid);
  assert.notEqual(replacement.files[0].path, oldFile);
  assert.equal(sha256(await readFile(oldFile)), oldHash);
  assert.equal(sha256(await readFile(replacement.files[0].path)), sha256(httpPayloads["resume.bin"]));
  t.diagnostic(JSON.stringify({ gid, committedBytes: paused.completedLength, sha256: sha256(httpPayloads["resume.bin"]), replacementName: path.basename(replacement.files[0].path) }));
});

test("290-02 SIGKILL recovery loads the same HTTP task paused and completes with the correct hash", candidateOnly, async (t) => {
  const harness = await createHarness();
  t.after(() => harness.close());
  const gid = await addPartialHttp(harness, "crash.bin");
  const { DatabaseSync } = await import("node:sqlite");
  // saveSession persists task identity; wait separately for the asynchronous native checkpoint.
  const checkpoint = await retry(() => {
    const database = new DatabaseSync(path.join(harness.stateDirectory, "stream", "state.db"), { readOnly: true });
    try {
      const row = database.prepare("SELECT completed_length FROM downloads WHERE gid = ?").get(gid);
      assert.ok(row?.completed_length > 0, "wait for committed native bytes before SIGKILL");
      return row.completed_length;
    } finally {
      database.close();
    }
  });
  await harness.rpc("aria2.saveSession");
  const before = (await harness.rpc("aria2.tellStatus", [gid])).result;
  const exited = once(harness.child, "exit");
  harness.child.kill("SIGKILL");
  await exited;
  await harness.startSidecar();
  await harness.waitForRpc("aria2.getVersion");
  const restored = await waitForTask(harness, gid, (task) => task.status === "paused");
  assert.equal(restored.files[0].path, before.files[0].path);
  assert.ok(Number(restored.completedLength) > 0, "a saved native checkpoint must retain valid bytes");
  assert.ok(Number(restored.completedLength) >= checkpoint, "recovery must retain the observed committed checkpoint");
  await assertStaysPaused(harness, gid);
  await harness.rpc("aria2.changeOption", [gid, { "max-download-limit": "0" }]);
  await harness.rpc("aria2.unpause", [gid]);
  await waitForStatus(harness, gid, "complete");
  assert.equal(sha256(await readFile(restored.files[0].path)), sha256(httpPayloads["resume.bin"]));
  t.diagnostic(JSON.stringify({ gid, committedCheckpoint: checkpoint, beforeKillBytes: before.completedLength, recoveredBytes: restored.completedLength }));
});

test("290-02 magnet keeps full metadata and selection offline before and after confirmation", candidateOnly, async (t) => {
  const torrent = createTorrent();
  const seed = await createHarness();
  t.after(() => seed.close());
  const { endpoint } = await startSeed(seed, torrent);
  const leecher = await createHarness();
  t.after(() => leecher.close());
  const metadataDirectory = path.join(leecher.root, "metadata");
  await mkdir(metadataDirectory);
  const { result: gid } = await leecher.rpc("aria2.addUri", [[`magnet:?xt=urn:btih:${torrent.infoHash}`], {
    dir: metadataDirectory, pause: "false", "pause-metadata": "true", "seed-time": "0",
  }]);
  await waitForStatus(leecher, gid, "active");
  await leecher.rpc("aria2.addBtPeers", [gid, [endpoint]]);
  const awaiting = await waitForTask(leecher, gid, (task) => task.bittorrent?.fileSelectionState === "awaiting");
  assert.equal(awaiting.status, "paused");
  assert.equal(awaiting.gid, gid);
  assert.ok(!awaiting.followedBy?.length);
  assert.equal(awaiting.completedLength, "0");
  await assertNoPayload(metadataDirectory, torrent);
  await assertNoPayload(leecher.directory, torrent);
  await saveAndStop(leecher);
  await saveAndStop(seed);
  await leecher.startSidecar();
  await leecher.waitForRpc("aria2.getVersion");
  const offline = await waitForTask(leecher, gid, (task) => task.status === "paused");
  assert.equal(offline.bittorrent?.fileSelectionState, "awaiting");
  assert.deepEqual(fileList(offline), fileList(awaiting));
  assert.ok((await readdir(leecher.stateDirectory, { recursive: true })).some((name) => name.endsWith(".fastresume")), "complete metadata must have native persisted state");
  await assertStaysPaused(leecher, gid);
  await assertNoPayload(metadataDirectory, torrent);
  await leecher.rpc("aria2.changeOption", [gid, { dir: leecher.directory, "select-file": "1" }]);
  const confirmed = await waitForTask(leecher, gid, (task) => task.bittorrent?.fileSelectionState === "ready");
  assert.equal(confirmed.status, "paused");
  assert.deepEqual(confirmed.files.map((file) => file.selected), ["true", "false"]);
  await saveAndRestart(leecher);
  const selectedOffline = await waitForTask(leecher, gid, (task) => task.status === "paused");
  assert.deepEqual(fileList(selectedOffline), fileList(confirmed));
  await assertStaysPaused(leecher, gid);

  await seed.startSidecar();
  await seed.waitForRpc("aria2.getVersion");
  const { gid: restartedSeedGid } = await startSeed(seed, torrent);
  await waitForTask(seed, restartedSeedGid, (task) => task.seeder === "true");
  const newEndpoint = await seedEndpoint(seed);
  await leecher.rpc("aria2.unpause", [gid]);
  await waitForStatus(leecher, gid, "active");
  await leecher.rpc("aria2.addBtPeers", [gid, [newEndpoint]]);
  await assertSelectedPayload(leecher, gid, torrent);
  t.diagnostic(JSON.stringify({ gid, offlineMetadata: true, selected: selectedOffline.files.map((file) => file.selected), selectedSha256: sha256(torrent.files[0].contents) }));
});

test("290-02 uploaded torrent survives an offline paused restart with original bytes and file selection", candidateOnly, async (t) => {
  const torrent = createTorrent();
  const harness = await createHarness();
  t.after(() => harness.close());
  const original = path.join(harness.root, "uploaded.torrent");
  await writeFile(original, torrent.bytes);
  const { result: gid } = await harness.rpc("aria2.addTorrent", [(await readFile(original)).toString("base64"), [], { pause: "true", "select-file": "2", "seed-time": "0" }]);
  const before = await waitForTask(harness, gid, (task) => task.status === "paused");
  await saveAndRestart(harness);
  const after = await waitForTask(harness, gid, (task) => task.status === "paused");
  assert.deepEqual(fileList(after), fileList(before));
  assert.deepEqual(after.files.map((file) => file.selected), ["false", "true"]);
  assert.equal(sha256(await readFile(original)), sha256(torrent.bytes));
  await assertStaysPaused(harness, gid);
});

for (const protocol of ["http", "socks4", "socks5"]) {
  test(`290-02 ${protocol} bt-proxy carries magnet metadata and selected payload through observed peer sockets`, candidateOnly, async (t) => {
    const torrent = createTorrent();
    const seed = await createHarness();
    t.after(() => seed.close());
    const { endpoint } = await startSeed(seed, torrent);
    const gateway = await createPeerGateway(endpoint);
    t.after(() => gateway.close());
    const proxy = await createProxyFixture(protocol, [gateway.port]);
    t.after(() => proxy.close());
    const harness = await createHarness([`--bt-proxy=${proxy.url}`]);
    t.after(() => harness.close());
    const { result: gid } = await harness.rpc("aria2.addUri", [[`magnet:?xt=urn:btih:${torrent.infoHash}`], { pause: "false", "pause-metadata": "true", "seed-time": "0" }]);
    await waitForStatus(harness, gid, "active");
    await harness.rpc("aria2.addBtPeers", [gid, [gateway.endpoint]]);
    await waitForTask(harness, gid, (task) => task.bittorrent?.fileSelectionState === "awaiting");
    await assertNoPayload(harness.directory, torrent);
    assertOnlyProxyConnections(gateway.connections, proxy.requests);
    await harness.rpc("aria2.changeOption", [gid, { "select-file": "1" }]);
    await harness.rpc("aria2.unpause", [gid]);
    await waitForStatus(harness, gid, "active");
    await harness.rpc("aria2.addBtPeers", [gid, [gateway.endpoint]]);
    await assertSelectedPayload(harness, gid, torrent);
    assertOnlyProxyConnections(gateway.connections, proxy.requests);
    t.diagnostic(JSON.stringify({ protocol, peerConnections: gateway.connections.length, proxyDestinations: proxy.requests.map(({ host, port }) => ({ host, port })) }));
  });

  test(`290-02 ${protocol} failed bt-proxy cannot fall back to a directly reachable peer`, candidateOnly, async (t) => {
    const torrent = createTorrent();
    const seed = await createHarness();
    t.after(() => seed.close());
    const { endpoint } = await startSeed(seed, torrent);
    const gateway = await createPeerGateway(endpoint);
    t.after(() => gateway.close());
    const proxy = await createProxyFixture(protocol, [gateway.port], { reject: true });
    t.after(() => proxy.close());
    const harness = await createHarness([`--bt-proxy=${proxy.url}`]);
    t.after(() => harness.close());
    const { result: gid } = await harness.rpc("aria2.addUri", [[`magnet:?xt=urn:btih:${torrent.infoHash}`], { pause: "false", "pause-metadata": "true" }]);
    await waitForStatus(harness, gid, "active");
    await harness.rpc("aria2.addBtPeers", [gid, [gateway.endpoint]]);
    await retry(() => assert.ok(proxy.requests.length > 0));
    await delay(1_000);
    assert.equal(gateway.connections.length, 0);
    const task = (await harness.rpc("aria2.tellStatus", [gid])).result;
    assert.notEqual(task.bittorrent?.fileSelectionState, "awaiting");
    assert.equal(task.completedLength, "0");
    await assertNoPayload(harness.directory, torrent);
  });

  for (const hostname of [false, true]) {
    test(`290-02 ${protocol} bt-proxy carries tracker and web-seed using ${hostname ? "destination hostname resolution" : "IP addresses"}`, candidateOnly, async (t) => {
      const base = createTorrent();
      const target = await createHttpFixture(Object.fromEntries(base.files.map((file) => [file.path, file.contents])));
      t.after(() => target.close());
      // .invalid names are synthetic fixture inputs: only the proxy fixture maps them to loopback.
      const trackerHost = hostname ? "tracker.motrix.invalid" : "127.0.0.1";
      const webseedHost = hostname ? "webseed.motrix.invalid" : "127.0.0.1";
      const torrent = createTorrent({ announce: `http://${trackerHost}:${target.port}/announce`, "url-list": [`http://${webseedHost}:${target.port}/`] });
      const proxy = await createProxyFixture(protocol, [target.port]);
      t.after(() => proxy.close());
      const harness = await createHarness([`--bt-proxy=${proxy.url}`]);
      t.after(() => harness.close());
      const { result: gid } = await harness.rpc("aria2.addTorrent", [torrent.bytes.toString("base64"), [], { pause: "false", "seed-time": "0" }]);
      try {
        await waitForStatus(harness, gid, "complete");
      } finally {
        t.diagnostic(JSON.stringify({ protocol, hostname, task: (await harness.rpc("aria2.tellStatus", [gid])).result, proxyRequests: proxy.requests, targetRequests: target.requests, output: harness.output }));
      }
      for (const file of torrent.files) assert.equal(sha256(await readFile(path.join(harness.directory, file.path))), sha256(file.contents));
      assert.ok(proxy.requests.some((entry) => entry.host === trackerHost));
      assert.ok(proxy.requests.some((entry) => entry.host === webseedHost));
      assert.ok(target.requests.some((entry) => entry.url.startsWith("/announce")));
      assert.ok(target.requests.some((entry) => entry.url === "/bt-fixture/first.bin"));
      assertOnlyProxyConnections(target.requests, proxy.requests);
    });
  }
}

test("290-02 switching bt-proxy while paused replaces connections and a failed switch blocks payload", { ...candidateOnly, timeout: 180_000 }, async (t) => {
  const torrent = createTorrent({}, 256 * 1024);
  const seed = await createHarness();
  t.after(() => seed.close());
  const { endpoint } = await startSeed(seed, torrent);
  const gateway = await createPeerGateway(endpoint);
  t.after(() => gateway.close());
  const first = await createProxyFixture("http", [gateway.port]);
  t.after(() => first.close());
  const failed = await createProxyFixture("socks5", [gateway.port], { reject: true });
  t.after(() => failed.close());
  const second = await createProxyFixture("socks5", [gateway.port]);
  t.after(() => second.close());
  const harness = await createHarness([`--bt-proxy=${first.url}`]);
  t.after(() => harness.close());
  const { result: gid } = await harness.rpc("aria2.addUri", [[`magnet:?xt=urn:btih:${torrent.infoHash}`], { pause: "false", "pause-metadata": "true", "seed-time": "0", "max-download-limit": "32K" }]);
  await waitForStatus(harness, gid, "active");
  await harness.rpc("aria2.addBtPeers", [gid, [gateway.endpoint]]);
  await waitForTask(harness, gid, (task) => task.bittorrent?.fileSelectionState === "awaiting");
  await harness.rpc("aria2.changeOption", [gid, { "select-file": "1" }]);
  await harness.rpc("aria2.unpause", [gid]);
  await waitForStatus(harness, gid, "active");
  await harness.rpc("aria2.addBtPeers", [gid, [gateway.endpoint]]);
  await waitForTask(harness, gid, (task) => Number(task.completedLength) >= 16_384 && Number(task.completedLength) < torrent.files[0].contents.length);
  await harness.rpc("aria2.pause", [gid]);
  const paused = await waitForTask(harness, gid, (task) => task.status === "paused");
  const oldRequests = first.requests.length;
  const oldConnections = gateway.connections.length;
  await harness.rpc("aria2.changeGlobalOption", [{ "bt-proxy": failed.url }]);
  await harness.rpc("aria2.unpause", [gid]);
  await waitForStatus(harness, gid, "active");
  await harness.rpc("aria2.addBtPeers", [gid, [gateway.endpoint]]);
  await retry(() => assert.ok(failed.requests.length > 0));
  await delay(1_000);
  assert.equal(first.requests.length, oldRequests);
  assert.equal(gateway.connections.length, oldConnections, "a failed proxy must not open a direct peer connection");
  const blocked = (await harness.rpc("aria2.tellStatus", [gid])).result;
  // Libtorrent may discard an unverified partial piece when the proxy reconnects.
  assert.ok(Number(blocked.completedLength) <= Number(paused.completedLength), "a failed proxy must not increase progress");
  t.diagnostic(JSON.stringify({ gid, beforeSwitchBytes: paused.completedLength, blockedBytes: blocked.completedLength }));
  await harness.rpc("aria2.pause", [gid]);
  await waitForStatus(harness, gid, "paused");
  await harness.rpc("aria2.changeGlobalOption", [{ "bt-proxy": second.url }]);
  await harness.rpc("aria2.changeOption", [gid, { "max-download-limit": "0" }]);
  await harness.rpc("aria2.unpause", [gid]);
  await waitForStatus(harness, gid, "active");
  await harness.rpc("aria2.addBtPeers", [gid, [gateway.endpoint]]);
  try {
    // A failed peer can wait (failcount + 1) * 60s before libtorrent retries it.
    await retry(async () => {
      const task = (await harness.rpc("aria2.tellStatus", [gid])).result;
      assert.notEqual(task.status, "error", JSON.stringify(task));
      assert.equal(task.files[0].completedLength, String(torrent.files[0].contents.length));
    }, 130_000);
  } finally {
    t.diagnostic(JSON.stringify({ task: (await harness.rpc("aria2.tellStatus", [gid])).result, firstRequests: first.requests, failedRequests: failed.requests, secondRequests: second.requests, peerConnections: gateway.connections, output: harness.output }));
  }
  await assertSelectedPayload(harness, gid, torrent);
  assert.ok(second.requests.length > 0);
  assertOnlyProxyConnections(gateway.connections, [...first.requests, ...second.requests]);
});

test("290-02 HTTPS bt-proxy is rejected without changing the current proxy", candidateOnly, async (t) => {
  const harness = await createHarness();
  t.after(() => harness.close());
  await harness.waitForRpc("aria2.getVersion");
  const before = (await harness.rpc("aria2.getGlobalOption")).result["bt-proxy"];
  await assert.rejects(() => harness.rpc("aria2.changeGlobalOption", [{ "bt-proxy": "https://127.0.0.1:9" }]), /bt-proxy|http.*socks4.*socks5/i);
  assert.equal((await harness.rpc("aria2.getGlobalOption")).result["bt-proxy"], before);
});

test("290-02 DHT is disabled by HTTP/SOCKS4 proxies and SOCKS5 sends it through UDP associate", candidateOnly, async (t) => {
  const target = await createUdpFixture();
  t.after(() => target.close());
  const args = ["--enable-dht=true", `--bt-dht-bootstrap-nodes=127.0.0.1:${target.port}`];
  const uri = `magnet:?xt=urn:btih:${createTorrent().infoHash}`;
  const direct = await createHarness(args);
  t.after(() => direct.close());
  const { result: directGid } = await direct.rpc("aria2.addUri", [[uri], { pause: "false" }]);
  await retry(() => assert.ok(target.packets.length > 0, "the direct positive control must reach the DHT fixture"), 15_000);
  await direct.rpc("aria2.pause", [directGid]);
  await waitForStatus(direct, directGid, "paused");
  await saveAndStop(direct);
  await delay(300);
  const baseline = target.packets.length;
  for (const protocol of ["http", "socks4", "socks5"]) {
    const proxy = await createProxyFixture(protocol, [target.port]);
    t.after(() => proxy.close());
    const harness = await createHarness([...args, `--bt-proxy=${proxy.url}`]);
    t.after(() => harness.close());
    const { result: gid } = await harness.rpc("aria2.addUri", [[uri], { pause: "false" }]);
    if (protocol === "socks5") {
      await retry(() => assert.ok(proxy.requests.some((entry) => entry.command === "udp" && !entry.rejected)), 15_000);
      assert.ok(target.packets.length > baseline);
      for (const packet of target.packets.slice(baseline)) {
        assert.ok(proxy.requests.some((entry) => entry.command === "udp" && entry.sourcePort === packet.remotePort), "DHT bypassed SOCKS5");
      }
    } else {
      await delay(1_000);
      assert.equal(target.packets.length, baseline, `${protocol} proxy must disable direct DHT`);
    }
    await harness.rpc("aria2.pause", [gid]);
    await waitForStatus(harness, gid, "paused");
    await saveAndStop(harness);
  }
  t.diagnostic(JSON.stringify({ directDhtPackets: baseline, socks5DhtPackets: target.packets.length - baseline }));
});

test("290-02 an unwritable state directory cannot report a successful download", candidateOnly, async (t) => {
  assert.notEqual(process.getuid(), 0, "permission failure must be exercised as a non-root user");
  const harness = await createHarness([], false);
  t.after(async () => { await chmod(harness.stateDirectory, 0o700); await harness.close(); });
  await chmod(harness.stateDirectory, 0o500);
  await harness.startSidecar();
  let ready = false;
  try {
    await harness.waitForRpc("aria2.getVersion");
    ready = true;
  } catch (error) {
    assert.match(`${error.message} ${harness.output}`, /state|permission|read.only|sqlite|denied/i);
    assert.ok(harness.child.exitCode !== null && harness.child.exitCode !== 0);
  }
  if (ready) {
    let gid;
    let rejected = false;
    try {
      gid = (await harness.rpc("aria2.addUri", [[harness.fixtureUrl("small.bin")], { pause: "false", media: "file" }])).result;
    } catch (error) {
      rejected = true;
      assert.match(`${error.message} ${harness.output}`, /state|permission|read.only|sqlite|denied/i);
    }
    if (!rejected) {
      assert.match(gid, /^[a-f0-9]+$/);
      await retry(async () => {
        const task = (await harness.rpc("aria2.tellStatus", [gid])).result;
        assert.equal(task.status, "error");
        assert.match(`${task.errorMessage} ${harness.output}`, /state|permission|read.only|sqlite|denied/i);
        t.diagnostic(JSON.stringify({ status: task.status, completedLength: task.completedLength, errorMessage: task.errorMessage }));
      });
    }
  }
  t.diagnostic(JSON.stringify({ rpcReady: ready, output: harness.output }));
});

async function createHarness(extraArgs = [], start = true) {
  const root = await mkdtemp(path.join(os.tmpdir(), "motrix-aria2-test-"));
  const directory = path.join(root, "downloads");
  const stateDirectory = path.join(root, "state");
  await mkdir(directory);
  await mkdir(stateDirectory, { mode: 0o700 });
  const session = path.join(root, "aria2.session");
  await writeFile(session, "");
  const fixture = await createHttpFixture();
  const harness = {
    root,
    directory,
    stateDirectory,
    fixture,
    child: null,
    args: extraArgs,
    output: "",
    port: await getFreePort(),
    btPort: await getFreePort(),
    secret: "motrix-test-secret",
    fixtureUrl(name) {
      return `http://127.0.0.1:${fixture.port}/${name}`;
    },
    async startSidecar(args = this.args) {
      const binary = await resolveBinary();
      this.args = args;
      this.output = "";
      this.spawnError = null;
      const defaults = [
        "--no-conf=true",
        "--enable-rpc=true",
        "--rpc-listen-all=false",
        `--rpc-listen-port=${this.port}`,
        `--rpc-secret=${this.secret}`,
        `--dir=${directory}`,
        `--save-session=${session}`,
        `--input-file=${session}`,
        "--pause=true",
        "--save-session-interval=1",
        "--file-allocation=none",
        "--enable-dht=false",
        "--enable-dht6=false",
        "--bt-enable-lpd=false",
        "--enable-peer-exchange=false",
        "--bt-interface=127.0.0.1",
        "--disable-ipv6=true",
        `--listen-port=${this.btPort}`,
        "--console-log-level=warn",
        ...(candidateDirectory ? [`--state-dir=${stateDirectory}`, "--bt-port-mapping=false", "--bt-transport=tcp", "--summary-interval=0"] : []),
      ];
      this.child = spawn(binary, [
        ...defaults.filter((value) => !args.some((override) => override.split("=")[0] === value.split("=")[0])),
        ...args,
      ], { stdio: ["ignore", "pipe", "pipe"], cwd: root });
      const capture = (chunk) => { this.output = (this.output + chunk.toString()).slice(-8_192); };
      this.child.stdout.on("data", capture);
      this.child.stderr.on("data", capture);
      this.child.on("error", (error) => { this.spawnError = error; capture(error.message); });
      this.exited = new Promise((resolve) => this.child.once("close", resolve));
    },
    async stopSidecar() {
      if (!this.child || this.spawnError || this.child.exitCode !== null || this.child.signalCode !== null) return { forced: false, code: this.child?.exitCode };
      this.child.kill("SIGTERM");
      await Promise.race([this.exited, delay(3_000)]);
      const forced = this.child.exitCode === null && this.child.signalCode === null;
      if (forced) this.child.kill("SIGKILL");
      await Promise.race([this.exited, delay(2_000).then(() => { throw new Error("sidecar did not exit after SIGKILL"); })]);
      return { forced, code: this.child.exitCode };
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
              try {
                const body = JSON.parse(Buffer.concat(chunks).toString("utf8"));
                if (body.error) reject(new Error(`${method}: ${body.error.message}`));
                else if ((response.statusCode ?? 500) >= 400) reject(new Error(`Aria2 RPC HTTP ${response.statusCode}: ${JSON.stringify(body)}`));
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
      try {
        await this.stopSidecar();
      } finally {
        await fixture.close();
        await rm(root, { recursive: true, force: true });
      }
    },
  };
  if (start) {
    try {
      await harness.startSidecar();
      await harness.waitForRpc("aria2.getVersion");
    } catch (error) {
      await harness.close();
      throw error;
    }
  }
  return harness;
}

async function waitForStatus(harness, gid, expected) {
  await retry(async () => {
    const response = await harness.rpc("aria2.tellStatus", [gid, ["status"]]);
    assert.equal(response.result?.status, expected);
  }, 15_000);
}

async function resolveBinary() {
  binaryPromise ??= (async () => {
    assert.ok(candidates[process.arch], `unsupported architecture: ${process.arch}`);
    if (!candidateDirectory) return path.resolve("assets", "aria2", `aria2-next-${process.arch === "arm64" ? "aarch64" : "x86_64"}-unknown-linux-gnu`);
    const candidate = candidates[process.arch];
    const binary = path.resolve(candidateDirectory, candidate.name);
    assert.equal(sha256(await readFile(binary)), candidate.hash, "candidate SHA-256 mismatch; refusing to execute");
    return binary;
  })();
  return binaryPromise;
}

async function addPartialHttp(harness, out) {
  await harness.waitForRpc("aria2.getVersion");
  const { result: gid } = await harness.rpc("aria2.addUri", [[harness.fixtureUrl("resume.bin")], {
    out, pause: "false", media: "file", "stream-max-connections": "1", "max-download-limit": "256K",
  }]);
  await waitForTask(harness, gid, (task) => Number(task.completedLength) >= 256 * 1024 && task.status === "active");
  return gid;
}

async function saveAndRestart(harness) {
  await saveAndStop(harness);
  await harness.startSidecar();
  await harness.waitForRpc("aria2.getVersion");
}

async function saveAndStop(harness) {
  await harness.rpc("aria2.saveSession");
  const stopped = await harness.stopSidecar();
  assert.equal(stopped.forced, false, "SIGKILL is not a successful graceful save");
  assert.equal(stopped.code, 0, `an abnormal exit is not a successful graceful save: ${harness.output}`);
}

async function waitForTask(harness, gid, predicate) {
  return retry(async () => {
    const task = (await harness.rpc("aria2.tellStatus", [gid])).result;
    assert.notEqual(task.status, "error", JSON.stringify(task));
    assert.ok(predicate(task), JSON.stringify(task));
    return task;
  }, 25_000);
}

async function assertStaysPaused(harness, gid) {
  const before = (await harness.rpc("aria2.tellStatus", [gid])).result;
  assert.equal(before.status, "paused");
  await delay(300);
  const after = (await harness.rpc("aria2.tellStatus", [gid])).result;
  assert.equal(after.status, "paused");
  assert.equal(after.completedLength, before.completedLength);
  assert.notEqual(after.seeder, "true");
}

async function assertNoPayload(directory, torrent) {
  for (const file of torrent.files) {
    await assert.rejects(() => stat(path.join(directory, file.path)), { code: "ENOENT" });
  }
  for (const entry of await readdir(directory, { recursive: true, withFileTypes: true })) {
    assert.equal(entry.isFile(), false, `payload or partfile was written before confirmation: ${entry.name}`);
  }
}

function fileList(task) {
  return task.files.map(({ index, path: filePath, length, selected }) => ({ index, path: filePath, length, selected }));
}

async function startSeed(harness, torrent) {
  for (const file of torrent.files) {
    const destination = path.join(harness.directory, file.path);
    await mkdir(path.dirname(destination), { recursive: true });
    await writeFile(destination, file.contents);
  }
  await harness.waitForRpc("aria2.getVersion");
  const { result: gid } = await harness.rpc("aria2.addTorrent", [torrent.bytes.toString("base64"), [], {
    pause: "false", "allow-overwrite": "true", "check-integrity": "true", "seed-time": "10",
  }]);
  await waitForTask(harness, gid, (task) => task.seeder === "true");
  return { gid, endpoint: await seedEndpoint(harness) };
}

async function seedEndpoint(harness) {
  return retry(async () => {
    const status = await harness.rpc("aria2.getBtSessionStatus");
    const endpoint = status.result.listenEndpoints.find((value) => value.startsWith("127.0.0.1:"));
    assert.ok(endpoint, JSON.stringify(status.result));
    return endpoint;
  });
}

async function assertSelectedPayload(harness, gid, torrent) {
  const task = await waitForTask(harness, gid, (value) => value.files[0]?.completedLength === String(torrent.files[0].contents.length));
  assert.equal(task.gid, gid);
  assert.deepEqual(task.files.map((file) => file.selected), ["true", "false"]);
  assert.equal(sha256(await readFile(task.files[0].path)), sha256(torrent.files[0].contents));
}

function assertOnlyProxyConnections(connections, requests) {
  assert.ok(connections.length > 0, "the controlled destination must observe network traffic");
  const proxyPorts = new Set(requests.map((entry) => entry.sourcePort));
  for (const connection of connections) assert.ok(proxyPorts.has(connection.remotePort), `direct connection bypassed proxy: ${connection.remotePort}`);
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
