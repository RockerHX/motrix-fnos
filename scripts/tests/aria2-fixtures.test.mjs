import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { once } from "node:events";
import { mkdtemp, rm } from "node:fs/promises";
import { connect } from "node:net";
import os from "node:os";
import path from "node:path";
import { setTimeout as delay } from "node:timers/promises";
import test from "node:test";
import { promisify } from "node:util";
import { bencode, createHttpFixture, createProxyFixture, createUdpFixture, httpPayloads } from "../extended/aria2-fixtures.mjs";

test("candidate mode rejects a missing Linux candidate or unsupported host instead of silently skipping", async (t) => {
  const directory = await mkdtemp(path.join(os.tmpdir(), "motrix-aria2-candidate-guard-"));
  t.after(() => rm(directory, { recursive: true, force: true }));
  const env = { ...process.env, MOTRIX_ARIA2_CANDIDATE_DIR: directory };
  // A nested test runner needs its own isolation rather than its parent's IPC context.
  delete env.NODE_TEST_CONTEXT;
  await assert.rejects(() => promisify(execFile)(process.execPath, ["--test", "--test-name-pattern=^sidecar starts", "scripts/extended/aria2-sidecar.test.mjs"], {
    env,
    timeout: 15_000,
  }), (error) => {
    assert.equal(error.code, 1);
    assert.match(`${error.stdout} ${error.stderr}`, process.platform === "linux" ? /ENOENT|unsupported architecture/ : /requires Linux/);
    return true;
  });
});

test("HTTP fixture preserves resource identity and serves exact byte ranges", async (t) => {
  const fixture = await createHttpFixture();
  t.after(() => fixture.close());
  const url = `http://127.0.0.1:${fixture.port}/resume.bin`;
  const response = await fetch(url, { headers: { range: "bytes=123-456" } });
  assert.equal(response.status, 206);
  assert.equal(response.headers.get("content-range"), `bytes 123-456/${httpPayloads["resume.bin"].length}`);
  assert.equal(response.headers.get("etag"), '"motrix-fixture-v1"');
  assert.deepEqual(Buffer.from(await response.arrayBuffer()), httpPayloads["resume.bin"].subarray(123, 457));
  // Keep the connection idle across several ticks to catch writes after response.end().
  await delay(50);
  const remainder = await fetch(url, { headers: { range: "bytes=4194300-" } });
  assert.deepEqual(Buffer.from(await remainder.arrayBuffer()), httpPayloads["resume.bin"].subarray(4194300));
  const invalid = await fetch(url, { headers: { range: "bytes=9999999-" } });
  assert.equal(invalid.status, 416);
  assert.equal((await fetch(`http://127.0.0.1:${fixture.port}/missing`)).status, 404);
});

test("torrent encoding sorts dictionary keys and preserves raw piece hashes", () => {
  assert.equal(bencode({ z: [1, "x"], a: Buffer.from([0, 255]) }).toString("hex"), "64313a61323a00ff313a7a6c693165313a786565");
  assert.throws(() => bencode(null), /unsupported/);
});

for (const protocol of ["http", "socks4", "socks5"]) {
  test(`${protocol} fixture handles fragmented negotiation and records the real upstream socket`, async (t) => {
    const target = await createHttpFixture();
    t.after(() => target.close());
    const proxy = await createProxyFixture(protocol, [target.port]);
    t.after(() => proxy.close());
    const socket = await openTunnel(t, protocol, proxy.port, target.port);
    const response = readUntil(socket, (bytes) => bytes.includes(httpPayloads["small.bin"]));
    socket.write("GET /small.bin HTTP/1.1\r\nHost: fixture\r\nConnection: close\r\n\r\n");
    assert.ok((await response).includes("HTTP/1.1 200"));
    assert.equal(proxy.requests.length, 1);
    assert.equal(proxy.requests[0].host, "127.0.0.1");
    assert.equal(proxy.requests[0].sourcePort, target.requests[0].remotePort);
  });

  test(`${protocol} fixture rejects a configured failure without contacting the target`, async (t) => {
    const target = await createHttpFixture();
    t.after(() => target.close());
    const proxy = await createProxyFixture(protocol, [target.port], { reject: true });
    t.after(() => proxy.close());
    await openTunnel(t, protocol, proxy.port, target.port, false);
    assert.equal(proxy.requests[0].rejected, true);
    assert.equal(target.requests.length, 0);
  });
}

for (const protocol of ["socks4", "socks5"]) {
  test(`${protocol} proxy resolves destination names at the proxy`, async (t) => {
    const target = await createHttpFixture();
    t.after(() => target.close());
    const proxy = await createProxyFixture(protocol, [target.port]);
    t.after(() => proxy.close());
    const socket = await openTunnel(t, protocol, proxy.port, target.port, true, "webseed.motrix.invalid");
    const response = readUntil(socket, (bytes) => bytes.includes(httpPayloads["small.bin"]));
    socket.write("GET /small.bin HTTP/1.1\r\nHost: fixture\r\nConnection: close\r\n\r\n");
    await response;
    assert.equal(proxy.requests[0].host, "webseed.motrix.invalid");
    assert.equal(proxy.requests[0].sourcePort, target.requests[0].remotePort);
  });
}

test("SOCKS5 UDP associate sends DHT-shaped packets only to allowed fixture ports", async (t) => {
  const target = await createUdpFixture();
  t.after(() => target.close());
  const client = await createUdpFixture();
  t.after(() => client.close());
  const proxy = await createProxyFixture("socks5", [target.port]);
  t.after(() => proxy.close());
  const socket = connect(proxy.port, "127.0.0.1");
  t.after(() => socket.destroy());
  await once(socket, "connect");
  const greeting = readUntil(socket, (bytes) => bytes.length === 2);
  socket.write(Buffer.from([5, 1, 0]));
  await greeting;
  const associate = readUntil(socket, (bytes) => bytes.length >= 10);
  socket.write(Buffer.from([5, 3, 0, 1, 0, 0, 0, 0, 0, 0]));
  const reply = await associate;
  assert.equal(reply[1], 0);
  const received = once(target.socket, "message", { signal: AbortSignal.timeout(3_000) });
  const payload = Buffer.from("controlled-dht-probe");
  const header = Buffer.from([0, 0, 0, 1, 127, 0, 0, 1, target.port >> 8, target.port & 255]);
  client.socket.send(Buffer.concat([header, payload]), reply.readUInt16BE(8), "127.0.0.1");
  assert.deepEqual((await received)[0], payload);
  assert.equal(proxy.requests[1].sourcePort, target.packets[0].remotePort);
  header[4] = 8;
  client.socket.send(Buffer.concat([header, payload]), reply.readUInt16BE(8), "127.0.0.1");
  await delay(30);
  assert.equal(proxy.requests[2].rejected, true);
  assert.equal(target.packets.length, 1);
});

test("HTTP proxy handles multiple requests on one connection and refuses external destinations", async (t) => {
  const target = await createHttpFixture();
  t.after(() => target.close());
  const proxy = await createProxyFixture("http", [target.port]);
  t.after(() => proxy.close());
  const socket = connect(proxy.port, "127.0.0.1");
  t.after(() => socket.destroy());
  await once(socket, "connect");
  const response = readUntil(socket, (bytes) => bytes.includes("d8:intervali1e5:peers0:e"));
  socket.write(`GET http://tracker.motrix.invalid:${target.port}/announce HTTP/1.1\r\nHost: tracker.motrix.invalid\r\nConnection: keep-alive\r\n\r\n`);
  assert.match((await response).toString(), /^HTTP\/1.1 200/);
  assert.equal(proxy.requests[0].host, "tracker.motrix.invalid");
  assert.equal(proxy.requests[0].sourcePort, target.requests[0].remotePort);
  const nextResponse = readUntil(socket, (bytes) => bytes.includes(httpPayloads["small.bin"]));
  socket.write(`GET http://webseed.motrix.invalid:${target.port}/small.bin HTTP/1.1\r\nHost: webseed.motrix.invalid\r\nConnection: keep-alive\r\n\r\n`);
  await nextResponse;
  assert.equal(proxy.requests[1].host, "webseed.motrix.invalid");
  assert.equal(proxy.requests[1].sourcePort, target.requests[1].remotePort);
  const denied = connect(proxy.port, "127.0.0.1");
  t.after(() => denied.destroy());
  await once(denied, "connect");
  const rejected = readUntil(denied, (bytes) => bytes.includes("502"));
  denied.write(`CONNECT example.com:${target.port} HTTP/1.1\r\n\r\n`);
  await rejected;
  assert.equal(proxy.requests[2].rejected, true);
  assert.equal(target.requests.length, 2);
});

async function openTunnel(t, protocol, proxyPort, targetPort, success = true, hostname = "127.0.0.1") {
  const socket = connect(proxyPort, "127.0.0.1");
  t.after(() => socket.destroy());
  await once(socket, "connect");
  if (protocol === "socks5") {
    const greeting = readUntil(socket, (bytes) => bytes.length >= 2);
    await writeFragments(socket, Buffer.from([5, 1, 0]));
    assert.deepEqual(await greeting, Buffer.from([5, 0]));
  }
  const named = hostname !== "127.0.0.1";
  const request = protocol === "http"
    ? Buffer.from(`CONNECT ${hostname}:${targetPort} HTTP/1.1\r\n\r\n`)
    : protocol === "socks4"
      ? Buffer.concat([Buffer.from([4, 1, targetPort >> 8, targetPort & 255, ...(named ? [0, 0, 0, 1] : [127, 0, 0, 1]), 0]), ...(named ? [Buffer.from(hostname), Buffer.from([0])] : [])])
      : Buffer.concat([Buffer.from([5, 1, 0]), named ? Buffer.concat([Buffer.from([3, hostname.length]), Buffer.from(hostname)]) : Buffer.from([1, 127, 0, 0, 1]), Buffer.from([targetPort >> 8, targetPort & 255])]);
  const response = readUntil(socket, (bytes) => protocol === "http" ? bytes.includes("\r\n\r\n") : bytes.length >= (protocol === "socks4" ? 8 : 10));
  await writeFragments(socket, request);
  const reply = await response;
  if (protocol === "http") assert.match(reply.toString(), success ? /^HTTP\/1.1 200/ : /^HTTP\/1.1 502/);
  else assert.equal(reply[1], protocol === "socks4" ? (success ? 90 : 91) : (success ? 0 : 1));
  return socket;
}

async function writeFragments(socket, bytes) {
  for (let offset = 0; offset < bytes.length; offset += 3) {
    socket.write(bytes.subarray(offset, offset + 3));
    await delay(1);
  }
}

function readUntil(socket, predicate) {
  return new Promise((resolve, reject) => {
    let bytes = Buffer.alloc(0);
    const timer = setTimeout(() => finish(new Error("fixture response timed out")), 3_000);
    const onData = (chunk) => {
      bytes = Buffer.concat([bytes, chunk]);
      if (predicate(bytes)) finish();
    };
    const onClose = () => finish(new Error("fixture socket closed before its response"));
    function finish(error) {
      clearTimeout(timer);
      socket.off("data", onData);
      socket.off("close", onClose);
      socket.off("error", finish);
      if (error) reject(error);
      else resolve(bytes);
    }
    socket.on("data", onData);
    socket.on("close", onClose);
    socket.on("error", finish);
  });
}
