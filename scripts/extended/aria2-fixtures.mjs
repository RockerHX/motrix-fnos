import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { createSocket } from "node:dgram";
import { createServer as createHttpServer, request as httpRequest } from "node:http";
import { connect, createServer as createTcpServer } from "node:net";

const resumePayload = Buffer.alloc(4 * 1024 * 1024);
for (let index = 0; index < resumePayload.length; index++) resumePayload[index] = index % 251;

export const httpPayloads = {
  "small.bin": Buffer.alloc(64 * 1024, 7),
  "large.bin": Buffer.alloc(256 * 1024, 9),
  "resume.bin": resumePayload,
};

export async function listen(server) {
  await new Promise((resolve, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", resolve);
  });
  const sockets = new Set();
  server.on("connection", (socket) => {
    sockets.add(socket);
    socket.on("error", () => socket.destroy());
    socket.on("close", () => sockets.delete(socket));
  });
  return {
    port: server.address().port,
    async close() {
      for (const socket of sockets) socket.destroy();
      await new Promise((resolve, reject) => server.close((error) => error ? reject(error) : resolve()));
    },
  };
}

export async function createHttpFixture(files = httpPayloads, peers = Buffer.alloc(0)) {
  const requests = [];
  const server = createHttpServer((request, response) => {
    requests.push({ url: request.url, range: request.headers.range, remotePort: request.socket.remotePort });
    if (request.url.startsWith("/announce")) {
      response.end(bencode({ interval: 1, peers }));
      return;
    }
    const payload = files[request.url.slice(1)];
    if (!payload) {
      response.writeHead(404).end();
      return;
    }
    const range = request.headers.range;
    const match = range?.match(/^bytes=(\d+)-(\d*)$/);
    const start = match ? Number(match[1]) : 0;
    const end = match?.[2] ? Math.min(Number(match[2]), payload.length - 1) : payload.length - 1;
    if ((range && !match) || start > end) {
      response.writeHead(416, { "content-range": `bytes */${payload.length}` }).end();
      return;
    }
    response.writeHead(range ? 206 : 200, {
      "content-type": "application/octet-stream",
      "content-length": end - start + 1,
      "accept-ranges": "bytes",
      etag: '"motrix-fixture-v1"',
      ...(range ? { "content-range": `bytes ${start}-${end}/${payload.length}` } : {}),
    });
    if (request.method === "HEAD") {
      response.end();
      return;
    }
    let offset = start;
    const timer = setInterval(() => {
      if (response.destroyed) return clearInterval(timer);
      const next = Math.min(offset + 32 * 1024, end + 1);
      response.write(payload.subarray(offset, next));
      offset = next;
      if (offset > end) {
        clearInterval(timer);
        response.end();
      }
    }, request.url === "/resume.bin" ? 15 : 1);
    response.on("close", () => clearInterval(timer));
  });
  return { ...await listen(server), requests };
}

export function bencode(value) {
  if (typeof value === "string") value = Buffer.from(value);
  if (Buffer.isBuffer(value)) return Buffer.concat([Buffer.from(`${value.length}:`), value]);
  if (Number.isSafeInteger(value)) return Buffer.from(`i${value}e`);
  if (Array.isArray(value)) return Buffer.concat([Buffer.from("l"), ...value.map(bencode), Buffer.from("e")]);
  assert.ok(value && typeof value === "object", "unsupported bencode value");
  return Buffer.concat([
    Buffer.from("d"),
    ...Object.keys(value).sort().flatMap((key) => [bencode(key), bencode(value[key])]),
    Buffer.from("e"),
  ]);
}

export function createTorrent(extra = {}, firstSize = 20_000) {
  const files = [
    { path: "bt-fixture/first.bin", contents: Buffer.from(Array.from({ length: firstSize }, (_, index) => index % 251)) },
    { path: "bt-fixture/second.bin", contents: Buffer.alloc(50_000, 31) },
  ];
  const payload = Buffer.concat(files.map((file) => file.contents));
  const pieces = [];
  for (let offset = 0; offset < payload.length; offset += 16_384) {
    pieces.push(createHash("sha1").update(payload.subarray(offset, offset + 16_384)).digest());
  }
  const info = {
    files: files.map((file) => ({ length: file.contents.length, path: [file.path.split("/")[1]] })),
    name: "bt-fixture",
    "piece length": 16_384,
    pieces: Buffer.concat(pieces),
  };
  return {
    files,
    bytes: bencode({ ...extra, info }),
    infoHash: createHash("sha1").update(bencode(info)).digest("hex"),
  };
}

export async function createPeerGateway(endpoint) {
  const connections = [];
  const upstreams = new Set();
  const [host, port] = endpoint.split(":");
  const fixture = await listen(createTcpServer((socket) => {
    connections.push({ remotePort: socket.remotePort });
    const upstream = connect({ host, port: Number(port) });
    upstreams.add(upstream);
    upstream.on("error", () => socket.destroy());
    upstream.on("close", () => upstreams.delete(upstream));
    socket.on("close", () => upstream.destroy());
    socket.pipe(upstream).pipe(socket);
  }));
  return {
    ...fixture,
    endpoint: `127.0.0.1:${fixture.port}`,
    connections,
    async close() {
      for (const upstream of upstreams) upstream.destroy();
      await fixture.close();
    },
  };
}

export async function createUdpFixture() {
  const socket = createSocket("udp4");
  const packets = [];
  socket.on("message", (bytes, remote) => packets.push({ bytes, remotePort: remote.port }));
  await new Promise((resolve, reject) => {
    socket.once("error", reject);
    socket.bind(0, "127.0.0.1", resolve);
  });
  return { socket, packets, port: socket.address().port, close: () => new Promise((resolve) => socket.close(resolve)) };
}

// Destination allowlist keeps malformed fixture requests from reaching any external service.
export async function createProxyFixture(protocol, allowedPorts, { reject = false } = {}) {
  assert.ok(["http", "socks4", "socks5"].includes(protocol));
  if (protocol === "http") return createHttpProxyFixture(allowedPorts, reject);
  const requests = [];
  const upstreams = new Set();
  const udp = protocol === "socks5" ? await createUdpFixture() : null;
  if (udp) {
    // shortcut: records outbound IPv4 DHT probes only; extend for DHT replies and IPv6 validation.
    udp.socket.on("message", (bytes) => {
      if (bytes.length < 10 || !bytes.subarray(0, 4).equals(Buffer.from([0, 0, 0, 1]))) return;
      const host = [...bytes.subarray(4, 8)].join(".");
      const port = bytes.readUInt16BE(8);
      const rejected = reject || host !== "127.0.0.1" || !allowedPorts.includes(port);
      requests.push({ host, port, command: "udp", rejected, sourcePort: udp.port });
      if (!rejected) udp.socket.send(bytes.subarray(10), port, "127.0.0.1");
    });
  }
  const server = createTcpServer((socket) => {
    let pending = Buffer.alloc(0);
    let greeting = protocol === "socks5";
    const onData = (chunk) => {
      pending = Buffer.concat([pending, chunk]);
      if (pending.length > 16_384) return socket.destroy();
      if (greeting) {
        if (pending.length < 2 || pending.length < 2 + pending[1]) return;
        if (pending[0] !== 5 || !pending.subarray(2, 2 + pending[1]).includes(0)) return socket.end(Buffer.from([5, 255]));
        socket.write(Buffer.from([5, 0]));
        pending = pending.subarray(2 + pending[1]);
        greeting = false;
      }
      let host, port, consumed;
      if (protocol === "socks4") {
        if (pending.length < 9) return;
        consumed = pending.indexOf(0, 8) + 1;
        if (!consumed) return;
        if (pending[0] !== 4 || pending[1] !== 1) return socket.destroy();
        port = pending.readUInt16BE(2);
        host = [...pending.subarray(4, 8)].join(".");
        if (host.startsWith("0.0.0.") && pending[7] !== 0) {
          const end = pending.indexOf(0, consumed);
          if (end === -1) return;
          host = pending.subarray(consumed, end).toString();
          consumed = end + 1;
        }
      } else {
        if (pending.length < 5) return;
        if (pending[0] !== 5 || ![1, 3].includes(pending[1]) || pending[2] !== 0) return socket.destroy();
        if (pending[3] === 1) {
          if (pending.length < 10) return;
          host = [...pending.subarray(4, 8)].join(".");
          consumed = 10;
        } else if (pending[3] === 3) {
          consumed = 7 + pending[4];
          if (pending.length < consumed) return;
          host = pending.subarray(5, consumed - 2).toString();
        } else return socket.destroy();
        port = pending.readUInt16BE(consumed - 2);
        if (pending[1] === 3) {
          requests.push({ host, port, command: "udp-associate", rejected: reject });
          socket.off("data", onData);
          socket.write(Buffer.from([5, reject ? 1 : 0, 0, 1, 127, 0, 0, 1, udp.port >> 8, udp.port & 255]));
          if (reject) socket.end();
          return;
        }
      }
      const record = { host, port, rejected: reject || !allowedPorts.includes(port) || !(host === "127.0.0.1" || host.endsWith(".motrix.invalid")) };
      requests.push(record);
      socket.off("data", onData);
      const reply = (success) => {
        if (protocol === "socks4") socket.write(Buffer.from([0, success ? 90 : 91, 0, 0, 0, 0, 0, 0]));
        else socket.write(Buffer.from([5, success ? 0 : 1, 0, 1, 127, 0, 0, 1, 0, 0]));
      };
      if (record.rejected) {
        reply(false);
        socket.end();
        return;
      }
      socket.pause();
      const upstream = connect({ host: "127.0.0.1", port });
      upstreams.add(upstream);
      upstream.on("error", () => { reply(false); socket.destroy(); });
      upstream.on("close", () => upstreams.delete(upstream));
      socket.on("close", () => upstream.destroy());
      upstream.on("connect", () => {
        record.sourcePort = upstream.localPort;
        reply(true);
        upstream.write(pending.subarray(consumed));
        socket.pipe(upstream).pipe(socket);
        socket.resume();
      });
    };
    socket.on("data", onData);
  });
  const fixture = await listen(server);
  return {
    ...fixture,
    url: `${protocol}://127.0.0.1:${fixture.port}`,
    requests,
    async close() {
      for (const upstream of upstreams) upstream.destroy();
      await fixture.close();
      if (udp) await udp.close();
    },
  };
}

async function createHttpProxyFixture(allowedPorts, reject) {
  const requests = [];
  const upstreams = new Set();
  const destination = (target) => {
    const url = new URL(target);
    const record = { host: url.hostname, port: Number(url.port || 80) };
    record.rejected = reject || url.protocol !== "http:" || !allowedPorts.includes(record.port) || !(record.host === "127.0.0.1" || record.host.endsWith(".motrix.invalid"));
    requests.push(record);
    return { url, record };
  };
  const trackSocket = (socket, record) => {
    upstreams.add(socket);
    socket.once("connect", () => { record.sourcePort = socket.localPort; });
    socket.once("close", () => upstreams.delete(socket));
  };
  const server = createHttpServer((request, response) => {
    let target;
    try { target = destination(request.url); } catch { return response.writeHead(400).end(); }
    if (target.record.rejected) return response.writeHead(502).end();
    const headers = { ...request.headers };
    for (const name of Object.keys(headers)) if (/^(proxy-|connection$)/i.test(name)) delete headers[name];
    const upstream = httpRequest({ hostname: "127.0.0.1", port: target.record.port, path: target.url.pathname + target.url.search, method: request.method, headers, agent: false }, (incoming) => {
      const headers = { ...incoming.headers };
      delete headers.connection;
      response.writeHead(incoming.statusCode, headers);
      incoming.pipe(response);
    });
    upstream.on("socket", (socket) => trackSocket(socket, target.record));
    upstream.on("error", () => {
      if (response.headersSent) response.destroy();
      else response.writeHead(502).end();
    });
    response.on("close", () => upstream.destroy());
    request.pipe(upstream);
  });
  server.on("connect", (request, socket, head) => {
    let target;
    try { target = destination(`http://${request.url}`); } catch { return socket.destroy(); }
    if (target.record.rejected) return socket.end("HTTP/1.1 502 Bad Gateway\r\nContent-Length: 0\r\n\r\n");
    const upstream = connect({ host: "127.0.0.1", port: target.record.port });
    trackSocket(upstream, target.record);
    upstream.on("error", () => socket.destroy());
    socket.on("close", () => upstream.destroy());
    upstream.once("connect", () => {
      socket.write("HTTP/1.1 200 Connection Established\r\n\r\n");
      upstream.write(head);
      socket.pipe(upstream).pipe(socket);
    });
  });
  const fixture = await listen(server);
  return { ...fixture, url: `http://127.0.0.1:${fixture.port}`, requests, async close() {
    for (const socket of upstreams) socket.destroy();
    await fixture.close();
  } };
}
