#!/usr/bin/env node
import { createHash } from "node:crypto";
import { mkdir, readFile, writeFile, chmod } from "node:fs/promises";
import { existsSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const VERSION = "2.4.9";
const TAG = `v${VERSION}`;
const BASE_URL = `https://github.com/AnInsomniacy/aria2-next/releases/download/${TAG}`;
const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");
const BIN_DIR = join(ROOT, "src-tauri", "binaries");

const assets = [
  {
    asset: `aria2-next-${VERSION}-macos-arm64`,
    target: "aria2-next-aarch64-apple-darwin",
  },
  {
    asset: `aria2-next-${VERSION}-linux-x86_64`,
    target: "aria2-next-x86_64-unknown-linux-gnu",
  },
  {
    asset: `aria2-next-${VERSION}-linux-aarch64`,
    target: "aria2-next-aarch64-unknown-linux-gnu",
  },
];

async function download(url) {
  const response = await fetch(url, { headers: { "User-Agent": "MotrixFnnas-dev" } });
  if (!response.ok) {
    throw new Error(`下载失败 ${response.status}: ${url}`);
  }
  return Buffer.from(await response.arrayBuffer());
}

function parseChecksums(text) {
  const result = new Map();
  for (const line of text.split(/\r?\n/)) {
    const match = line.trim().match(/^([a-f0-9]{64})\s+(.+)$/i);
    if (match) {
      result.set(match[2].trim(), match[1].toLowerCase());
    }
  }
  return result;
}

function sha256(buffer) {
  return createHash("sha256").update(buffer).digest("hex");
}

await mkdir(BIN_DIR, { recursive: true });

const checksumAsset = `aria2-next-${VERSION}-checksums.sha256`;
const checksumText = (await download(`${BASE_URL}/${checksumAsset}`)).toString("utf8");
await writeFile(join(BIN_DIR, checksumAsset), checksumText);
const checksums = parseChecksums(checksumText);

for (const item of assets) {
  const expected = checksums.get(item.asset);
  if (!expected) {
    throw new Error(`checksums.sha256 中找不到 ${item.asset}`);
  }

  const targetPath = join(BIN_DIR, item.target);
  let data;
  if (existsSync(targetPath)) {
    data = await readFile(targetPath);
  } else {
    data = await download(`${BASE_URL}/${item.asset}`);
    await writeFile(targetPath, data);
  }

  const actual = sha256(data);
  if (actual !== expected) {
    throw new Error(`${item.asset} SHA256 不匹配: expected ${expected}, got ${actual}`);
  }

  await chmod(targetPath, 0o755);
  console.log(`${item.target} OK ${actual}`);
}
