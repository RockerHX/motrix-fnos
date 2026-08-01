import { describe, expect, it } from "vitest";
import { isRfc1918Hostname, lanJsonRpcEndpoint } from "./lanJsonRpcEndpoint";

describe("lanJsonRpcEndpoint", () => {
  it("builds a concrete endpoint only for RFC1918 IPv4 hostnames", () => {
    for (const hostname of ["10.0.0.2", "172.16.0.1", "172.31.255.254", "192.168.1.12"]) {
      expect(isRfc1918Hostname(hostname)).toBe(true);
      expect(lanJsonRpcEndpoint(hostname)).toEqual({
        value: `http://${hostname}:17082/jsonrpc`,
        concrete: true,
      });
    }
  });

  it("uses a non-copyable placeholder for domains and disallowed addresses", () => {
    for (const hostname of ["motrix.example.com", "127.0.0.1", "169.254.1.1", "172.32.0.1", "::1"]) {
      expect(lanJsonRpcEndpoint(hostname)).toEqual({
        value: "http://<飞牛局域网IP>:17082/jsonrpc",
        concrete: false,
      });
    }
  });
});
