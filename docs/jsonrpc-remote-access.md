# JSON-RPC 公网接入指南

> 用途：让外部解析站通过反向代理提交下载任务；文件仍由 NAS 上的 Aria2 Next 下载。
> 本文只保留长期有效的配置和验证步骤。域名、Token、证书私钥和个人网络参数不要写入仓库。

## 1. 端口边界

| 端口 | 用途 | 访问范围 |
| --- | --- | --- |
| `17080` | Web 管理面、HTTP API、SSE | fnOS 管理入口；业务 API 需要 Web Session |
| `17081` | 回环 JSON-RPC | 仅 NAS 本机和本机反向代理；Lucky 应代理到这里 |
| `17082` | 局域网 JSON-RPC | 仅 RFC1918 IPv4 客户端；使用独立 Token |
| `6800` | Aria2 内部 RPC | 仅 Rust server 使用，不对外开放 |

公网入口只提供 `/jsonrpc`。不要把 `17080`、`17082` 或 `6800` 直接暴露到公网。

## 2. 推荐拓扑

```text
外部解析站 / 浏览器
  -> wss://motrix.example.com:8443/jsonrpc
  -> Cloudflare（橙云）
  -> 家庭公网 IPv6
  -> Lucky TLS 服务（8443）
  -> http://127.0.0.1:17081/jsonrpc
  -> Motrix Rust server
  -> 127.0.0.1:6800 Aria2 Next RPC
```

说明：

- HTTPS 页面调用 WebSocket 时使用 `wss://`，不要使用 `ws://`。
- Cloudflare、Lucky 只负责 TLS 和反向代理，不直接代理 Aria2 的 `6800`。
- 如果家庭 IPv4 不支持入站连接，DNS 只维护可入站的 AAAA 记录。
- `17081` 绑定回环地址；NAS 局域网客户端不能直接访问它，这是预期行为。

## 3. 配置前检查

1. 安装并启动 Motrix FPK，确认管理服务就绪。
2. 在设置页配置 JSON-RPC Token。该 Token 与 Web 管理密码、局域网 Token、Aria2 RPC Secret 都不同。
3. 准备一个 DNS 名称，并让它指向 NAS 的公网地址。Cloudflare 代理必须保持橙云状态。
4. Lucky 与 Motrix 必须处于同一网络命名空间，或使用 NAS 内部可达地址；不要把 `17081` 加入端口映射。
5. Cloudflare 建议使用 `Full (strict)`，源站 TLS 服务绑定 Origin CA 或其他受信任证书。

## 4. NAS 本地验证

### 4.1 服务就绪

`/api/app/ready` 只用于生命周期探测，不代表已经登录管理面：

```bash
curl -i http://NAS地址:17080/api/app/ready
```

任务、设置、日志和授权目录接口请从 fnOS/FN Connect 的管理入口登录后使用。

### 4.2 回环 JSON-RPC

在 NAS 本机执行：

```bash
curl -i http://127.0.0.1:17081/jsonrpc \
  -H 'Content-Type: application/json' \
  -d '{
    "jsonrpc": "2.0",
    "id": "test-version",
    "method": "aria2.getVersion",
    "params": []
  }'
```

成功返回 Aria2 版本后，再配置 Lucky。`17081` 不能通过 NAS 局域网地址代替回环地址访问。

### 4.3 局域网 JSON-RPC（可选）

从真实 RFC1918 IPv4 客户端访问 `http://NAS地址:17082/jsonrpc`。局域网入口关闭、来源不是 RFC1918 IPv4 或 Token 不匹配时，请求应被拒绝；不要用 `X-Forwarded-For` 伪造来源。

## 5. Lucky 与 Cloudflare

### 5.1 DNS

- 创建一条 Motrix 专用记录，类型通常为 `AAAA`，值为 NAS 当前可入站的全局 IPv6。
- 开启 Cloudflare 橙云，只保留一条同名 AAAA；不要把共享出口或不可入站 IPv4 写成源站。
- Cloudflare API Token 只授予目标 Zone 的读取和 DNS 编辑权限，不使用全局 Token。
- 多级托管域名需要在 Lucky 中配置正确的自定义后缀；出现 `zone not found` 时先检查 Zone 与主机记录拆分。
- 橙云开启后，公共 `dig` 查询返回 Cloudflare 地址而不是家庭 IPv6，这是正常现象；真实源站值以 Cloudflare 控制台和 Lucky 日志为准。

### 5.2 TLS 服务

Lucky 前端监听 HTTPS（示例端口 `8443`），启用 TLS 1.2 或更高版本，并绑定覆盖请求域名的源站证书：

```text
外部地址：motrix.example.com:8443
TLS：启用
Cloudflare：Full (strict)
```

Cloudflare `526` 通常表示源站证书不匹配、过期或未绑定到 Lucky；临时切换 `Full` 只用于定位问题，修复后应恢复 `Full (strict)`。

### 5.3 反向代理

Lucky 子规则的后端必须是：

```text
http://127.0.0.1:17081
```

不要填写 `17080`。管理端口即使被错误代理，也不应成为公网 RPC 入口；发现公网根路径或 `/api/*` 能打开时，应立即修正 Lucky 配置。

## 6. 外网验证

以下示例把 `motrix.example.com:8443` 替换为实际域名和端口。

### 6.1 管理面隔离

```bash
curl -i https://motrix.example.com:8443/
curl -i https://motrix.example.com:8443/api/settings
```

两个请求应返回 `404`，或由上游网关返回 `403`；不能打开 Web UI、任务 API 或设置 API。

### 6.2 JSON-RPC HTTP

```bash
curl -i https://motrix.example.com:8443/jsonrpc \
  -H 'Content-Type: application/json' \
  -d '{
    "jsonrpc": "2.0",
    "id": "test-version",
    "method": "aria2.getVersion",
    "params": []
  }'
```

### 6.3 CORS 预检

```bash
curl -i -X OPTIONS https://motrix.example.com:8443/jsonrpc \
  -H 'Origin: https://sender.example.com' \
  -H 'Access-Control-Request-Method: POST' \
  -H 'Access-Control-Request-Headers: content-type'
```

响应应包含 RPC 所需的 `Access-Control-Allow-*` 头；管理 API 不应因此开放跨域访问。

### 6.4 WebSocket

```bash
npx wscat -c wss://motrix.example.com:8443/jsonrpc
```

连接后发送：

```json
{"jsonrpc":"2.0","id":"test","method":"aria2.getVersion","params":[]}
```

### 6.5 添加测试任务

只有在保存目录已由 fnOS 授权后，才使用测试文件验证 `aria2.addUri`：

```bash
curl -i https://motrix.example.com:8443/jsonrpc \
  -H 'Content-Type: application/json' \
  -d '{
    "jsonrpc": "2.0",
    "id": "test-add",
    "method": "aria2.addUri",
    "params": [
      "token:替换为 JSON-RPC Token",
      ["https://speed.cloudflare.com/__down?bytes=1048576"],
      {"dir": "/已授权目录", "out": "remote-test.bin"}
    ]
  }'
```

成功返回 GID。测试完成后删除测试任务和文件。

## 7. 解析站填写

```text
主机地址：wss://motrix.example.com:8443/jsonrpc
RPC 密钥：Motrix 设置页中的公网 JSON-RPC Token
保存路径：fnOS 已授权目录
```

如果 Lucky 使用标准 HTTPS 端口 `443`，URL 中可以省略端口。公网解析站不要填写 `17080`、`17081`、`17082` 或 `6800`。

## 8. 常见故障

| 现象 | 处理 |
| --- | --- |
| NAS 本机 `17081` 失败 | 确认 server 已就绪，并从 NAS 本机执行；局域网地址不能替代 `127.0.0.1` |
| 公网根路径或 `/api/*` 能打开 | Lucky 错误代理到 `17080`；改为 `http://127.0.0.1:17081` |
| Lucky 连接不上 `127.0.0.1` | 检查网络命名空间；让 Lucky 与 Motrix 共享网络，或使用 NAS 内部地址 |
| `zone not found` | 检查 Cloudflare Token 的 Zone 权限和 Lucky 多级域名后缀 |
| `526 Invalid SSL certificate` | 检查源站证书域名、有效期、私钥匹配和 Lucky 绑定；修复后使用 `Full (strict)` |
| `522` 或连接超时 | 检查公网 IPv6、NAS/Lucky 的 `8443` 防火墙和 Cloudflare AAAA；删除不可入站 A 记录 |
| CORS 或 WebSocket 失败 | 依次检查 `OPTIONS`、`POST`、`wscat`；确认外部页面使用 `https` 时对应 `wss` |
| `aria2.addUri` 返回 Token 错误 | 在 Motrix 设置页配置公网 Token，并确认解析站使用同一 Token；它不是 Aria2 RPC Secret |

## 9. 安全清单

- `aria2.getVersion` 可用于连通性测试；`aria2.addUri` 和 `system.multicall` 中的写操作必须携带正确 Token。
- 公网 Token、局域网 Token、Web 管理密码和 Aria2 RPC Secret 分开保存，不能互相替代。
- 不把 Token、密码、证书私钥或 Cloudflare API Token 写入仓库、截图、日志或诊断包。
- Cloudflare API Token 使用最小 Zone 权限；Lucky 只反代到回环 RPC 端口。
- Token 为空时，公网地址仍可能返回 `getVersion`，但不能添加任务；这不表示鉴权已关闭。

## 10. 参考资料

- Cloudflare 动态 DNS：https://developers.cloudflare.com/dns/manage-dns-records/how-to/managing-dynamic-ip-addresses/
- Cloudflare Origin CA：https://developers.cloudflare.com/ssl/origin-configuration/origin-ca/
- Cloudflare 代理端口：https://developers.cloudflare.com/fundamentals/reference/network-ports/
- Lucky DDNS：https://lucky666.cn/docs/modules/ddns
