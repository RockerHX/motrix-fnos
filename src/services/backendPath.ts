const FNOS_GATEWAY_PREFIX = "/app/motrix";

export function backendPath(path: string) {
  if (
    window.location.pathname === FNOS_GATEWAY_PREFIX ||
    window.location.pathname.startsWith(`${FNOS_GATEWAY_PREFIX}/`)
  ) {
    return `${FNOS_GATEWAY_PREFIX}${path}`;
  }
  return path;
}
