# FN Connect 替代 Lucky 提供 JSON-RPC 的实验计划

> 状态：域名入口已实机验证；FN Connect 作为公网 JSON-RPC 入口仍待验证，不代表当前支持  
> 记录日期：2026-07-15  
> 安全要求：实验不得记录或提交 FN Connect Cookie、Web 密码、Session、CSRF、JSON-RPC Token、Aria2 secret 或设备完整配置。

## 1. 实验目标

验证 FN Connect 为应用入口生成的三级域名，能否在不依赖 Lucky、Cloudflare 和额外 DDNS 配置的情况下，为外部 Aria2 客户端提供稳定的 HTTP 与 WebSocket JSON-RPC 服务。

当前实机观察：

- 旧版入口 `motrix.fnos.main` 生成 `motrix-fnos-main.<account>.fnos.net`。
- 只清空默认入口仍会生成 `motrix-main.<account>.fnos.net`，说明 `.url` 入口身份仍参与域名注册。
- 将 `appname`、`desktop_appname` 和唯一 `.url` 入口统一为 `motrix` / `motrix.Application`，并保持空 `desktop_applaunchname` 后，实机确认 `motrix.<account>.fnos.net` 可打开 Motrix。
- 由于包身份已变化，新 FPK 按全新应用处理，不承担旧 `motrix.fnos` 安装的数据和升级迁移。

本实验只验证技术可行性。验证完成前继续保持当前安全拓扑：

```text
FN Connect 应用入口 -> 17080 -> Web UI、管理 API、SSE
Lucky 公网入口      -> 17081 -> 仅 JSON-RPC
```

## 2. 已确认事实与未知项

### 已确认

- `manifest.service_port` 和唯一的 `app/ui/config` 入口只注册管理端口 `17080`。
- `motrix` 是新的 `appname`，`motrix.Application` 同时是 `desktop_appname` 和唯一 `.url` 入口键；`desktop_applaunchname` 留空，域名已实机验证。
- JSON-RPC 专用 listener 当前只绑定 `127.0.0.1:17081`，未注册为 fnOS 平台端口。
- `v1.7.2` 及更早的单 listener 版本曾在 `17080` 同时提供 Web UI、管理 API 与 `/jsonrpc`。
- FN Connect 登录态、Motrix Web Session 和 JSON-RPC Token 是不同凭据，不能互相替代。
- Motrix Web Session Cookie 使用 `SameSite=Strict`；跨站网页不能假设可以携带该 Session。

### 待验证

1. FN Connect 三级域名是否把 HTTP、OPTIONS 和 WebSocket 全部透明转发到入口端口。
2. 已登录 FN Connect 的浏览器从第三方域名发起 WSS 时，是否会携带并被接受 FN Connect 登录态。
3. 原生 Aria2/Motrix 客户端能否在没有浏览器 Cookie 的情况下通过 FN Connect。
4. FN Connect 退出、Session 过期或设备离线时，已有 HTTP/WSS 连接如何结束。
5. FN Connect 或系统代理日志是否可能记录 JSON-RPC 请求体中的 Token。

## 3. 域名命名调查

根据当前实机结果，项目采用以下已验证映射：

```text
旧入口 motrix.fnos.main                         -> motrix-fnos-main.<account>.fnos.net
Motrix 入口 motrix.main、默认入口为空            -> motrix-main.<account>.fnos.net
Motrix 入口 motrix.Application、默认入口为空     -> motrix.<account>.fnos.net
Lucky 入口与 desktop_appname=Lucky.Application  -> lucky.<account>.fnos.net
```

`appname`、`display_name`、前端运行时的 `appName` 和 `.url` 入口 ID 是不同字段。能够在浏览器控制台看到 `trim.download-center`，不代表 FN Connect 一定注册了 `trim-download-center.<account>.fnos.net`。

后续在 NAS 上只读检查 Lucky、Motrix 与下载中心的入口声明：

```bash
sudo find /var/apps -path '*/app/ui/config' -type f \
  -exec sh -c 'echo "--- $1"; sed -n "1,160p" "$1"' sh {} \;

sudo find /var/apps -name manifest -type f \
  -exec sh -c 'echo "--- $1"; grep -E "^(appname|desktop_appname|desktop_applaunchname|service_port)" "$1"' sh {} \;
```

记录时只保留应用名、入口 ID、入口类型、端口和是否存在网关字段，不提交设备路径中的用户信息或其他应用配置全文。

Lucky 2.27.2 FPK 已确认 `.url` 键和 `desktop_appname` 均为 `Lucky.Application`，`desktop_applaunchname` 为空，且 `config/resource` 没有域名或网关声明。Motrix 的两轮实测确认：入口键使用 `motrix.Application` 是短域名生效的关键；`config/resource`、端口和反向代理不是决定因素。

后续重点比较：

- FN Connect 的 HTTP、OPTIONS 和 WebSocket 是否都能稳定转发到 Motrix 管理端口。
- 下载中心实际的 `.url` 键是 `trim.download-center`、`trim.download-center2`，还是没有独立端口入口。
- 系统下载中心是否通过路径、统一网关或系统桌面内部路由打开，而不是 FN Connect 独立三级域名。

## 4. JSON-RPC 实验构建

实验不得直接修改正式包。应创建临时测试分支和测试 FPK，在保留 `17081` 的同时，为管理 listener 临时增加精确的 `/jsonrpc` 路由，便于对照：

```text
17080 /jsonrpc -> 实验入口
17081 /jsonrpc -> 当前基准入口
```

第一轮只使用 JSON-RPC Token 保护写操作，不要求 Motrix Web Session 或 CSRF。这样可以验证 FN Connect 是否能替代 Lucky，而不会把“客户端不会登录 Web API”误判为 FN Connect 转发失败。

实验风险：局域网 `NAS:17080/jsonrpc` 会同时恢复，因此必须使用临时强随机 Token，实验结束后立即轮换，并在测试完成后恢复正式包。

## 5. 验证矩阵

### 5.1 基线

在安装实验包前记录：

- 当前版本、设备架构与 fnOS 版本。
- `17080`、`17081` 的监听地址。
- Lucky HTTP/WSS JSON-RPC 是否正常。
- 当前任务、设置和 Aria2 session 是否正常保留。

不得记录 Token 原文。

### 5.2 同源浏览器测试

先登录 FN Connect，再直接打开 Motrix 三级域名。在该页面 DevTools Console 中执行同源只读请求：

```javascript
fetch("/jsonrpc", {
  method: "POST",
  headers: { "Content-Type": "application/json" },
  body: JSON.stringify({
    jsonrpc: "2.0",
    id: "version",
    method: "aria2.getVersion",
    params: [],
  }),
}).then(async (response) => ({ status: response.status, body: await response.text() }));
```

预期：已登录 FN Connect 时到达实验 `/jsonrpc`；退出 FN Connect 或使用无痕窗口时被 FN Connect 拒绝，不能到达 Motrix。

### 5.3 跨域 HTTP 与 WebSocket

从实际使用的第三方网页或扩展分别验证：

- `POST https://motrix.<account>.fnos.net/jsonrpc`（待实机确认）
- `OPTIONS https://motrix.<account>.fnos.net/jsonrpc`（待实机确认）
- `wss://motrix.<account>.fnos.net/jsonrpc`（待实机确认）

矩阵必须覆盖：

| FN Connect 状态 | Token | HTTP | WSS | 预期 |
| --- | --- | --- | --- | --- |
| 已登录 | 正确 | 待测 | 待测 | 只读与添加任务成功 |
| 已登录 | 错误 | 待测 | 待测 | 写操作拒绝 |
| 已退出 | 正确 | 待测 | 待测 | FN Connect 层拒绝 |
| Session 过期 | 正确 | 待测 | 待测 | 新请求和重连失败 |

特别记录响应是 JSON、重定向、HTML 登录页、HTTP 401/403/404，还是 WebSocket 握手失败。

### 5.4 原生客户端

在不导入浏览器 Cookie的前提下，让原生 Motrix/Aria2 客户端仅配置三级域名和 JSON-RPC Token。

如果原生客户端无法通过 FN Connect 登录层，则 FN Connect 不能作为通用 JSON-RPC 公网入口，即使同一浏览器内的 WSS 可以工作。

### 5.5 边界与日志

- 局域网访问 `NAS:17080/jsonrpc` 的实际行为。
- 公网三级域名的根路径、管理 API 和 SSE 是否仍受 Motrix Web 密码保护。
- FN Connect 退出后已有 WebSocket 是否断开。
- Token 轮换后旧 Token 是否立即失效。
- fnOS、FN Connect 和应用日志是否出现 Token 或完整请求体。

## 6. 是否可以删除 Lucky 的判定标准

只有同时满足以下条件，才能考虑删除 Lucky 与额外 DDNS：

1. 实际使用的远程客户端均能完成 HTTP/WSS 调用，不需要手工复制 Cookie。
2. FN Connect 未登录、退出或过期后请求可靠失败。
3. JSON-RPC Token 缺失、错误和轮换行为正确。
4. 公网不能绕过 Motrix Web 密码访问管理 API、任务、设置、路径、日志或 SSE。
5. FN Connect 和系统代理不记录 Token 原文。
6. 可以接受或消除 `17080/jsonrpc` 在局域网重新可达的影响。
7. ARM 与 x86 实机行为一致，升级和回滚不会破坏任务数据。

任一条件未满足，继续使用 Lucky 将公网请求定向到回环 `17081`。

## 7. 飞牛下载中心与 Aria2 调查

“下载中心使用 Aria2”与“下载中心公开了可复用的 Aria2 JSON-RPC 服务”不是同一件事。后续只能执行无破坏性检查：

```bash
ps -ef | grep -E '[a]ria2|download-center'
ss -lntp
sudo find /var/apps -maxdepth 5 \( -type s -o -name '*aria2*' \)
```

检查时不得停止系统进程、修改配置、读取或输出 RPC secret。需要确认：

- 是否确实存在 Aria2/派生进程。
- 是否启用了 RPC，使用 TCP 还是 Unix Socket。
- RPC 是否只绑定回环、是否使用动态端口和随机 secret。
- fnOS 是否提供稳定、受支持的任务 API，而不是仅供系统前端使用的私有接口。
- 第三方 FPK 用户是否有权限访问对应进程、Socket、数据库和下载目录。
- 系统升级是否会改变接口、认证或数据结构。

Motrix 当前 Web UI 不直接依赖公开 `/jsonrpc`，而是调用自身 `/api/*`；Rust server 再通过内部 JSON-RPC 控制随包提供的 Aria2 Next。公网 `/jsonrpc` 只是给第三方客户端使用的兼容入口。

即使飞牛下载中心内部使用 Aria2，只有在存在官方支持、稳定鉴权且第三方应用有权限使用的控制接口时，才值得评估复用。直接连接系统应用的私有 RPC、提取 secret 或读写其数据库不作为可接受方案。

## 8. 回滚

1. 停止并卸载实验包，恢复当前正式 FPK。
2. 确认管理 listener 恢复为未知路径 404，RPC 只存在于回环 `17081`。
3. 轮换实验期间使用的 JSON-RPC Token。
4. 恢复 Lucky 后端并验证 HTTP、CORS 与 WSS。
5. 确认任务、设置、授权目录和 Aria2 session 未变化。

实验结果应追加到本文档，但失败或未验证项目必须明确标记，不得根据浏览器页面可打开推断 HTTP、WSS 或原生客户端已经通过。

## 9. 参考资料

- 飞牛官方应用入口：https://developer.fnnas.com/docs/core-concepts/app-entry/
- 飞牛官方统一网关：https://developer.fnnas.com/docs/core-concepts/gateway-registration/
- 本仓库 FPK 入口与实机约束：`docs/fpk-packaging.md`
- 本仓库双监听器与鉴权边界：`docs/architecture.md`
