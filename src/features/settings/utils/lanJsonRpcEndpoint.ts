export interface LanJsonRpcEndpoint {
  value: string;
  concrete: boolean;
}

export function isRfc1918Hostname(hostname: string) {
  const octets = hostname.split(".").map(Number);
  if (
    octets.length !== 4 ||
    octets.some((octet) => !Number.isInteger(octet) || octet < 0 || octet > 255)
  ) {
    return false;
  }
  return (
    octets[0] === 10 ||
    (octets[0] === 172 && octets[1]! >= 16 && octets[1]! <= 31) ||
    (octets[0] === 192 && octets[1] === 168)
  );
}

export function lanJsonRpcEndpoint(hostname: string): LanJsonRpcEndpoint {
  if (isRfc1918Hostname(hostname)) {
    return { value: `http://${hostname}:17082/jsonrpc`, concrete: true };
  }
  return { value: "http://<飞牛局域网IP>:17082/jsonrpc", concrete: false };
}
