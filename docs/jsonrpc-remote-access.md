# JSON-RPC 公网接入与排障记录

> 记录日期：2026-07-05  
> 最近验证：2026-07-14
> 目标：在外网通过解析站的 Aria2 发送能力，把任务提交到 NAS 上的 Motrix FNOS，由 NAS 本地 Aria2 Next 下载。

## 1. 最终推荐拓扑

```text
解析站 / 外网浏览器
  -> wss://motrix.<your-domain>:8443/jsonrpc
  -> Cloudflare 代理（AAAA / 橙云）
  -> 家宽公网 IPv6
  -> Lucky Web 服务（TLS / 8443）
  -> http://127.0.0.1:17080/jsonrpc
  -> motrix-fnos Rust server
  -> 127.0.0.1:6800 Aria2 Next RPC
```

说明：

- 外网只负责“提交任务”，文件下载仍由 NAS 自己走家宽。
- 解析站推荐使用 `wss://.../jsonrpc`，避免 HTTPS 页面调用 `ws://` 被浏览器拦截。
- Lucky 只做反向代理，不直接暴露 Aria2 的 `6800`。
- Cloudflare 可通过 AAAA 记录回源到家里的公网 IPv6；没有公网 IPv4 不影响该方案。
- 公网开放 `/jsonrpc` 前，必须在 Motrix 设置页配置 JSON-RPC 密钥；该密钥不是 Aria2 RPC Secret，保存后立即生效。

## 2. 不推荐 FN Connect 直连

测试过以下地址：

```bash
curl -i https://rockerhx.fnos.net/jsonrpc
curl -i https://motrix-fnos-main.rockerhx.fnos.net/jsonrpc
```

结果分别出现：

- `302` 跳转到 `https://fnos.net/rockerhx`
- `403 FN Connect 暂无权限访问该服务`

结论：

- FN Connect 应用子域名依赖飞牛登录态 / 权限网关。
- 浏览器从飞牛桌面进入应用时能访问，不代表第三方解析站能直接调用。
- 第三方解析站不会携带你的飞牛登录 Cookie，因此不适合作为 Aria2 JSON-RPC 公网入口。

## 3. 先验证 NAS 本地服务

### 3.1 Motrix FNOS 后端

```bash
curl -i http://192.168.1.12:17080/api/aria2/rpc
curl -i http://192.168.1.12:17080/api/tasks
```

期望：

```json
{"connected":true,"version":"2.4.9"}
```

### 3.2 授权保存目录

```bash
curl -s http://192.168.1.12:17080/api/storage/accessible-paths
```

示例：

```json
{"paths":["/vol1/1000/tmp"]}
```

### 3.3 本地 JSON-RPC

```bash
curl -i http://192.168.1.12:17080/jsonrpc \
  -H 'Content-Type: application/json' \
  -d '{
    "jsonrpc": "2.0",
    "id": "test-version",
    "method": "aria2.getVersion",
    "params": []
  }'
```

成功后再配置公网反代。

### 3.4 配置 JSON-RPC 密钥

在 Motrix FNOS 设置页找到“JSON-RPC 密钥”：

1. 手动输入一串密钥，或点击“随机生成”。
2. 保存设置后立即生效，无需重启 Aria2。
3. 后续解析站的“RPC 密钥”填写同一串密钥。

说明：

- 该密钥只用于 Motrix FNOS 的 `/jsonrpc` 添加任务鉴权。
- 它不是 Aria2 RPC Secret，不会传给 `127.0.0.1:6800`。
- 留空时，`aria2.addUri` 会返回 `JSON-RPC token not configured`，不会添加任务。

## 4. Cloudflare 配置

### 4.1 当前两个 DDNS 域名的职责

当前使用两套相互独立的 DDNS：

| 域名 | 更新方 | Cloudflare 状态 | 用途 |
| --- | --- | --- | --- |
| `nas.wiwby.de5.net` | fnOS 自带 DDNS | A / AAAA 均为“仅 DNS” | IPv6 网络直接访问 NAS |
| `motrix.andy-singapore.ccwu.cc` | Lucky DDNS | AAAA 开启橙云代理 | IPv4-only / IPv6 网络通过 Cloudflare 访问 Motrix |

两条记录可以指向同一个 NAS 公网 IPv6，但访问效果并不相同：

- `nas.wiwby.de5.net` 返回家庭真实 A / AAAA，访问者直接连接 NAS；家庭 IPv4 不可入站时，公司 IPv4-only 网络无法依靠该 A 记录访问。
- `motrix.andy-singapore.ccwu.cc` 对外返回 Cloudflare Anycast A / AAAA，访问者先连接 Cloudflare，再由 Cloudflare 通过家庭 IPv6 回源到 Lucky `8443`。
- DNS 只决定目标地址，不决定端口、TLS 证书和 Lucky 子规则；`https://nas...` 与 `https://motrix...:8443` 不是等价 URL。

fnOS 自带 DDNS 使用的 Token 只授权 `wiwby.de5.net`。Lucky 更新 `andy-singapore.ccwu.cc` 时需要另一个覆盖该 Zone 的 Token；也可以扩大同一 Token 的 Zone 范围，但不推荐把两个应用共用一个高权限 Token。

### 4.2 创建 Lucky 专用 Cloudflare API Token

进入 Cloudflare “API 令牌 -> 创建自定义令牌”，填写：

```text
令牌名称：Lucky DDNS - Motrix

权限：
  区域 / 区域 / 读取
  区域 / DNS / 编辑

区域资源：
  包括 / 特定区域 / andy-singapore.ccwu.cc

客户端 IP 地址筛选：留空
```

注意：

- Lucky 官方文档要求 Cloudflare 使用“区域 Token”，不能使用全局 API Token。
- 家庭公网 IPv6 会变化，不能在 Token 上绑定固定客户端 IP。
- Token 只在创建时完整显示一次，保存到密码管理器；不得写入本仓库、截图或日志。
- 如果 Cloudflare 控制台显示的 Zone 名称与本文不同，以实际包含 `motrix` 记录的 Zone 为准。

### 4.3 Lucky 多级域名后缀

`andy-singapore.ccwu.cc` 属于多级托管域名。先进入：

```text
Lucky -> 动态域名 -> 设置
```

在“自定义多级域名后缀列表”中添加：

```text
ccwu.cc
```

这样 Lucky 才能把：

```text
完整域名：motrix.andy-singapore.ccwu.cc
Zone：andy-singapore.ccwu.cc
主机记录：motrix
```

正确拆分。若未配置，常见结果是 `zone not found`、无法找到域名记录，或请求了错误 Zone。

### 4.4 添加 Lucky DDNS 任务

进入“动态域名 -> 任务列表 -> 添加任务”，填写：

| 字段 | 当前配置 / 建议值 |
| --- | --- |
| 任务名称 | `motrix-cloudflare-ipv6` |
| 任务开关 | 启用 |
| 调试模式 | 首次配置启用；稳定后关闭 |
| 操作模式 | 简易模式 |
| 托管服务商 | Cloudflare |
| Token | 4.2 创建的专用 Token |
| 强制同步 | `3600` 秒 |
| 首次执行任务延迟 | `16` 秒 |
| 检测周期 | `36–60` 秒 |
| `{ipv6Addr}` | 启用 |
| `{ipv4Addr}` | 禁用 |
| 全局 Webhook / Webhook | 禁用 |

只启用 IPv6，避免把运营商共享出口 IPv4 写成 Motrix 源站。Lucky 检测到的 `{ipv6Addr}` 必须与 fnOS 网卡上的全局 IPv6 一致：

- 可用地址通常以 `2xxx:` 开头。
- 不得使用 `fe80::` 链路本地地址。
- NAS 同时存在多个全局 IPv6 时，以实际可以从公网连接、且 Lucky Web 服务正在监听的地址为准。
- 当前实测 Lucky 获取地址与 fnOS `eth0` 第一条全局 IPv6 一致。

### 4.5 添加同步记录

在任务中点击“添加同步记录”：

```text
记录状态：启用
完整域名：motrix.andy-singapore.ccwu.cc
记录类型：AAAA
记录值：{ipv6Addr}
线路：默认
TTL：Auto（Cloudflare API 中通常为 1）
Cloudflare 代理 / CDN / Proxied：启用（橙云）
```

Cloudflare 中只保留一条同名 AAAA 记录。Lucky 会按域名和类型查找并更新现有记录，不要创建多个同名 AAAA。

保存后点击“手动触发同步”，任务日志应显示：

```text
成功识别 Zone
成功获取公网 IPv6
找到或创建 motrix AAAA
同步成功
```

随后核对 Cloudflare 控制台：AAAA 内容与 NAS 当前公网 IPv6 相同，并且代理状态仍为橙云。还应等待一次自动检测或 fnOS IPv6 变化后再次检查，确认 Lucky 更新记录时不会把橙云改回“仅 DNS”。

### 4.6 DNS 验证

橙云开启后，公共 DNS 查询返回 Cloudflare Anycast 地址，而不是 NAS 真实 IPv6，这是正常结果：

```bash
dig @1.1.1.1 A motrix.andy-singapore.ccwu.cc
dig @1.1.1.1 AAAA motrix.andy-singapore.ccwu.cc
```

当前实测：

```text
A     -> Cloudflare 104.21.* / 172.67.*
AAAA  -> Cloudflare 2606:4700:*
TTL   -> 约 300 秒
```

因此不能通过 `dig` 对比家庭 IPv6 来判断 Lucky 是否写入正确；应查看 Lucky 日志和 Cloudflare 控制台中 AAAA 的源站内容。

`nas.wiwby.de5.net` 当前为“仅 DNS”，查询结果会直接显示家庭真实 A / AAAA。检测到一个公网形式的 IPv4 不代表该 IPv4 支持入站连接；家宽 CGNAT / 共享出口仍可能无法从公网访问。

### 4.7 SSL/TLS：边缘证书与源服务器证书

推荐 Cloudflare Zone 配置：

```text
SSL/TLS 模式：完全（严格） / Full (strict)
```

HTTPS 实际使用两层证书：

```text
浏览器
  -> Cloudflare 边缘证书（Universal SSL）
Cloudflare
  -> Cloudflare Origin CA 源服务器证书
Lucky / NAS / VPS
```

#### 边缘证书

- 用于浏览器到 Cloudflare 的 TLS。
- 由浏览器信任的公共 CA 签发。
- 有效期较短是正常现象；Cloudflare 自动签发、续期、轮换和部署，无需下载或安装到 NAS。
- 控制台中的“通用”和“备份”证书都属于 Cloudflare 托管证书。
- 只要 Zone 有效、Universal SSL 未关闭且 DNS 保持橙云，一般不需要人工维护。

#### Origin CA 源服务器证书

当前 Lucky 使用的证书覆盖：

```text
*.andy-singapore.ccwu.cc
andy-singapore.ccwu.cc
```

有效期到 2041 年。它只用于 Cloudflare 到 Lucky 的回源加密：

- 必须把证书和私钥安装到实际终止 TLS 的 Lucky / NAS / VPS，并绑定到对应服务。
- Cloudflare 在 `Full (strict)` 模式下验证其有效期和域名。
- Origin CA 不受普通浏览器信任；一旦把记录切成灰云让浏览器直连，浏览器仍会报证书不受信任。
- 同一通配符私钥复制到多台源服务器虽然可以工作，但会扩大泄露影响。不同源站建议分别创建 Origin证书。

如果 Cloudflare 返回：

```text
Error 526 Invalid SSL certificate
```

处理顺序：

1. 确认 Origin证书覆盖请求域名；
2. 确认证书和私钥匹配；
3. 确认 Lucky Web 主规则绑定了正确证书；
4. 临时改为 `Full` 只能用于定位问题，长期保持 `Full (strict)`。

#### `nas.wiwby.de5.net` 证书方案

当 `nas` 保持灰云时，浏览器直接看到 fnOS 443 的证书。fnOS 当前的 `*.rockerhx.fnos.net` 和自签名 `fnOS` 证书不覆盖 `nas.wiwby.de5.net`，因此浏览器报错属于预期行为。

如需让 `nas` 采用与 Motrix 相同的 Cloudflare 模型：

1. 在 `wiwby.de5.net` Zone 创建覆盖以下主机名的 Origin证书：

   ```text
   *.wiwby.de5.net
   wiwby.de5.net
   ```

2. 将证书和私钥导入 fnOS；
3. 在 fnOS“安全性 -> 证书 -> 服务配置”中绑定到 Web 管理服务；
4. 将 `nas` AAAA 改成橙云；
5. 将 `wiwby.de5.net` Zone 的 SSL/TLS 模式设置为 `Full (strict)`。

家庭 IPv4 不可入站时，`nas` 不应同时保留一个不可用的橙云 A 源站。优先让 DDNS 只维护 AAAA；若 fnOS 自带 DDNS不能关闭 IPv4同步，可改由 Lucky 创建另一个 IPv6-only DDNS任务。

如果 `nas` 要保持灰云直连，则不能使用 Origin CA 解决浏览器信任问题，应在 fnOS安装并绑定 Let's Encrypt 等公共 CA 证书。

### 4.8 端口

当前使用：

```text
8443
```

Cloudflare 支持代理 HTTPS `8443`，访问时必须带端口：

```text
https://motrix.andy-singapore.ccwu.cc:8443/
wss://motrix.andy-singapore.ccwu.cc:8443/jsonrpc
```

如果后续改用 `443`，可以省略端口：

```text
wss://motrix.andy-singapore.ccwu.cc/jsonrpc
```

参考：

- Lucky DDNS：https://lucky666.cn/docs/modules/ddns
- Cloudflare 动态更新 DNS：https://developers.cloudflare.com/dns/manage-dns-records/how-to/managing-dynamic-ip-addresses/
- Cloudflare 代理状态：https://developers.cloudflare.com/dns/proxy-status/
- Cloudflare Origin CA：https://developers.cloudflare.com/ssl/origin-configuration/origin-ca/
- Cloudflare 支持端口：https://developers.cloudflare.com/fundamentals/reference/network-ports/
- Cloudflare WebSocket：https://developers.cloudflare.com/network/websockets/

## 5. Lucky 配置

### 5.1 Web 服务主规则

```text
规则名称：motrix-jsonrpc
规则开关：启用
操作模式：简易模式
监听类型：IPv6（或 IPv4 + IPv6）
监听端口：8443
防火墙自动放行：开启
TLS：启用
TLS最低版本：TLS1.2
证书：Cloudflare Origin Certificate
CorazaWAF：无
```

IPv6 家宽通常没有传统 NAT，但仍可能有路由器 / 光猫防火墙，需要放行：

```text
WAN IPv6 -> NAS IPv6 TCP 8443
```

能访问 `:5666` 只代表 `5666` 放行，不代表 `8443` 也放行（但是实际上5666这种奇怪端口都没管，8443也就没管，可能联通的光猫的ipv6管理比较随意，所有端口都放开了）。

### 5.2 反向代理子规则

```text
服务类型：反向代理
前端地址：motrix.andy-singapore.ccwu.cc
后端地址：http://127.0.0.1:17080
CorazaWAF：无
万事大吉：启用
忽略后端TLS证书验证：无所谓，后端是 http
使用目标地址Host请求头：否
自动反代理重定向：否
记录访问日志：启用
```

注意：

- 前端地址不要带 `https://`。
- fnOS 当前实测 Lucky 与 Motrix 可通过 `http://127.0.0.1:17080` 通信，优先使用回环地址，避免依赖 NAS 局域网地址变化。
- 如果以后改成 Docker / 隔离环境，`127.0.0.1` 可能指向 Lucky 自身；此时改用 `http://192.168.1.12:17080` 或实际 NAS 局域网地址。

当前实测配置使用：

```text
前端域名：motrix.andy-singapore.ccwu.cc
外部端口：8443
后端地址：http://127.0.0.1:17080
```

`POST /jsonrpc` 已通过 Cloudflare、家庭 IPv6 和 Lucky 反代返回 Aria2 Next `2.4.9`，证明当前 Lucky 与 Motrix 可以通过回环地址通信。

## 6. 外网测试命令

### 6.1 页面连通性

```bash
curl -i https://motrix.andy-singapore.ccwu.cc:8443/api/app/info
```

或浏览器打开：

```text
https://motrix.andy-singapore.ccwu.cc:8443/
```

### 6.2 JSON-RPC getVersion

```bash
curl -i https://motrix.andy-singapore.ccwu.cc:8443/jsonrpc \
  -H 'Content-Type: application/json' \
  -d '{
    "jsonrpc": "2.0",
    "id": "test-version",
    "method": "aria2.getVersion",
    "params": []
  }'
```

期望：

```json
{
  "jsonrpc": "2.0",
  "id": "test-version",
  "result": {
    "version": "2.4.9",
    "enabledFeatures": []
  }
}
```

### 6.3 CORS 预检

```bash
curl -i -X OPTIONS https://motrix.andy-singapore.ccwu.cc:8443/jsonrpc \
  -H 'Origin: https://mf.dp.wpurl.cc' \
  -H 'Access-Control-Request-Method: POST' \
  -H 'Access-Control-Request-Headers: content-type'
```

期望看到类似：

```text
Access-Control-Allow-Origin: *
Access-Control-Allow-Methods: GET, POST, OPTIONS
Access-Control-Allow-Headers: content-type, authorization
```

### 6.4 WebSocket

```bash
npx wscat -c wss://motrix.andy-singapore.ccwu.cc:8443/jsonrpc
```

连接后发送：

```json
{"jsonrpc":"2.0","id":"test","method":"aria2.getVersion","params":[]}
```

### 6.5 添加测试任务

```bash
curl -i https://motrix.andy-singapore.ccwu.cc:8443/jsonrpc \
  -H 'Content-Type: application/json' \
  -d '{
    "jsonrpc": "2.0",
    "id": "test-add",
    "method": "aria2.addUri",
    "params": [
      "token:你的密钥",
      ["https://speed.cloudflare.com/__down?bytes=10485760"],
      {
        "dir": "/vol1/1000/tmp",
        "out": "remote-test-10mb.bin",
        "split": "64",
        "max-connection-per-server": "64",
        "min-split-size": "1M"
      }
    ]
  }'
```

成功返回 Aria2 GID。

## 7. 解析站配置

如果 Lucky 监听 `8443`：

```text
主机地址：wss://motrix.andy-singapore.ccwu.cc:8443/jsonrpc
RPC密钥：填写设置页保存的 JSON-RPC 密钥
保存路径：/vol1/1000/tmp
最大连接：256
```

如果 Lucky 改为 `443`：

```text
主机地址：wss://motrix.andy-singapore.ccwu.cc/jsonrpc
```

## 8. 常见故障对照表

| 现象 | 判断 | 处理 |
| --- | --- | --- |
| Lucky 显示 `zone not found` | 多级域名拆分错误或 Token Zone 不匹配 | 在 Lucky 自定义多级域名后缀添加 `ccwu.cc`，确认 Token 授权 `andy-singapore.ccwu.cc` |
| Lucky 显示同步成功但 IPv6 错误 | 获取到链路本地、代理出口或错误网卡地址 | 对比 fnOS `eth0` 全局 IPv6，排除 `fe80::` 和代理接口 |
| `dig` 返回 Cloudflare IP 而不是家庭 IPv6 | 橙云代理的正常行为 | 到 Cloudflare 控制台或 Lucky 日志检查真实 AAAA 内容 |
| Lucky 更新后记录变成灰云 | 同步记录未开启 Proxied 或更新器重建记录 | 编辑同步记录启用 Cloudflare代理，并在下一次自动同步后复查 |
| Cloudflare `526 Invalid SSL certificate` | Cloudflare 到 Lucky 的 TLS 证书不可信或不匹配 | Lucky 导入并绑定 Cloudflare Origin Certificate；或临时改 Cloudflare SSL 为 `Full` |
| `nas.wiwby.de5.net` 灰云直连证书报错 | fnOS 提供的证书不覆盖该域名或不受浏览器信任 | 灰云使用公共 CA 证书；或安装 Origin证书、绑定服务并开启橙云和 `Full (strict)` |
| 橙云 `nas` 偶发 `522` | Cloudflare 可能尝试不可入站的家庭 IPv4源站 | 删除不可用 A，让 DDNS 只维护 AAAA |
| 能访问 `:5666`，但 `:8443` 不通 | 只证明 `5666` 开放，不代表 `8443` 开放 | 路由器/光猫 IPv6 防火墙放行 NAS TCP 8443 |
| Lucky 子规则无流量 | 请求没到 Lucky 或域名没匹配 | 检查 DNS、Cloudflare 代理状态、Lucky 前端地址不要带协议 |
| Lucky 反代 `127.0.0.1` 不通 | Lucky 与 Motrix 不在同一网络命名空间 | 改后端为 `http://192.168.1.12:17080` |
| FN Connect 返回 403 | 飞牛公网应用入口需要登录态 | 不用 FN Connect 做第三方解析站 JSON-RPC 入口，改用 Lucky |
| 页面能开但 `/jsonrpc` 失败 | 路由或 CORS/WebSocket 问题 | 分别测试 `/api/app/info`、`POST /jsonrpc`、`OPTIONS /jsonrpc`、`wscat` |

## 9. 安全注意

当前 `/jsonrpc` 兼容入口为了适配解析站，支持：

- `aria2.getVersion`：匿名可用，方便连通性测试；
- `aria2.addUri`：必须带正确 `token:<密钥>`；
- `system.multicall`：其中每个 `aria2.addUri` 子调用都必须带正确 token；
- 透传常用 Aria2 加速参数，例如 `split`、`max-connection-per-server`、`min-split-size`、`user-agent`、`header`、`referer`。

公网使用要求：

- 必须在设置页配置 JSON-RPC 密钥；
- 解析站 RPC 密钥填写同一串密钥；
- 保存设置后立即生效，无需重启 Aria2；
- 该密钥不是 Aria2 RPC Secret，不要暴露真实 Aria2 RPC secret；
- 不要提交证书私钥、token 或 Cloudflare API Token 到仓库。
- Cloudflare API Token 使用最小 Zone 权限；fnOS DDNS 与 Lucky DDNS建议使用不同 Token。
- Cloudflare边缘证书由 Cloudflare自动续期，不需要下载或部署到源站。
- Origin CA 私钥只保存在需要终止 TLS 的源服务器，不要无必要地复制到多台机器。

如果 JSON-RPC 密钥为空，公网地址即使可访问，也不能通过 `/jsonrpc` 添加下载任务。

## 10. DNS 与端口理解

DNS 只负责把域名解析到 IP，例如：

```text
nas.example.com -> 2408:....
```

但以下地址不是严格等价：

```text
https://nas.example.com:5667/login
http://[2408:...]:5666/login
```

差异包括：

- 协议不同：`https` vs `http`
- 端口不同：`5667` vs `5666`
- Host 头不同：域名 vs IP
- TLS 证书校验不同

所以一个端口能访问，不代表另一个端口也开放。

同理，两个域名绑定同一个 IPv6 也不代表效果相同：

```text
nas.wiwby.de5.net
  -> 灰云时直接访问 NAS 443

motrix.andy-singapore.ccwu.cc:8443
  -> Cloudflare 橙云
  -> NAS IPv6:8443
  -> Lucky
  -> Motrix 17080
```

判断访问行为时必须同时检查：DNS代理状态、协议、端口、Host、TLS终止位置和反向代理后端。
