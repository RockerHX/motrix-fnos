import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import { parseChecksums, sha256 } from "../lib/script-utils.mjs";

const checksums = parseChecksums(readFileSync("assets/aria2/aria2-next-2.8.6-checksums.sha256", "utf8"));
const dependencyManifest = readFileSync("assets/aria2/aria2-next-2.8.6-dependencies.env", "utf8");

test("2.8.6 native 依赖材料保留固定版本和提交来源", () => {
  assert.match(dependencyManifest, /Source: https:\/\/github\.com\/AnInsomniacy\/aria2-next\/blob\/v2\.8\.6\/packaging\/dependencies\.env/);
  for (const version of ["CURL_VERSION=8.21.0", "NGHTTP2_VERSION=1.70.0", "LIBTORRENT_VERSION=2.1.1", "BOOST_VERSION=1.91.0", "OPENSSL_VERSION=3.5.6", "FFMPEG_VERSION=8.1.2"]) {
    assert.match(dependencyManifest, new RegExp(`^${version}$`, "m"));
  }
});

for (const [architecture, machine] of [["x86_64", 62], ["aarch64", 183]]) {
  test(`内置 ${architecture} sidecar 与 2.8.6 checksum 和 ELF 架构一致`, () => {
    const binary = readFileSync(`assets/aria2/aria2-next-${architecture}-unknown-linux-gnu`);
    const expected = checksums.get(`aria2-next-2.8.6-linux-${architecture}`);
    assert.match(expected, /^[a-f0-9]{64}$/);
    assert.equal(sha256(binary), expected);
    assert.equal(binary.subarray(0, 4).toString("hex"), "7f454c46");
    assert.equal(binary[4], 2, "必须是 64 位 ELF");
    assert.equal(binary[5], 1, "必须使用 little-endian");
    assert.equal(binary.readUInt16LE(18), machine);
  });
}
