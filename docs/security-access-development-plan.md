# 管理面鉴权与 JSON-RPC 公网隔离开发计划

> 状态：待评审，未开始实施  
> 编写日期：2026-07-14  
> 优先级：P0，公网 JSON-RPC 正式开放前必须完成

## 1. 背景与问题

当前 Rust server 只监听一个端口，FPK 环境为：

```text
0.0.0.0:17080
```

同一个 Axum Router 同时注册：

```text
Web UI 静态资源
/api/*
/api/events
/jsonrpc
```

Lucky 当前把公网域名完整反向代理到 `http://127.0.0.1:17080`，因此访问者不仅能调用 `/jsonrpc`，也能匿名访问 Web UI、任务 API、设置、授权目录、Aria2 控制和调试日志。

JSON-RPC Token 只保护 JSON-RPC 写操作，不能保护 `/api/*`。当前 `/api/settings` 还会向 Web UI 返回 JSON-RPC Token，因此不能把 Token 视为管理面的替代鉴权。

本项目尚处开发阶段，没有外部用户与旧版兼容负担。本次改造直接建立安全默认值，不保留 `17080/jsonrpc` 兼容入口。

## 2. 已确认的目标架构

### 2.1 双监听器

```text
管理监听器
0.0.0.0:17080
├── Web UI
├── /api/auth/*
├── /api/*（需要 Web Session）
├── /api/events（需要 Web Session）
└── 不注册 /jsonrpc

RPC 专用监听器
127.0.0.1:17081
├── GET /jsonrpc       WebSocket
├── POST /jsonrpc      JSON-RPC HTTP
├── OPTIONS /jsonrpc   CORS 预检
└── 其他路径统一 404
```

固定边界：

- `17080` 只承担管理面，不再提供 JSON-RPC。
- `17081` 只绑定回环地址，不加入 fnOS 端口转发，不允许局域网或公网直接连接。
- Lucky 只反向代理到 `http://127.0.0.1:17081`。
- 公网域名根路径、`/api/*`、静态资源和 SSE 在应用层不存在，不依赖来源 IP、Host 或代理 Header 判断。
- JSON-RPC 保留独立 Token；Web管理密码和 JSON-RPC Token 是两套不同凭据。

### 2.2 Web管理鉴权

管理面采用单管理员密码与服务端 Session：

```text
浏览器输入管理密码
  -> POST /api/auth/login
  -> 服务端验证 Argon2id 哈希
  -> 设置 HttpOnly Session Cookie
  -> Web UI / API / SSE 使用同源 Session
```

安全默认值：

- 新版本首次启动进入“待初始化”状态，必须设置 Web管理密码后才能访问任务和设置。
- 不保留匿名管理作为默认状态。
- 设置页仍提供“Web管理访问保护”开关，但关闭操作必须验证当前密码并显示明确风险提示。
- 关闭保护只影响 `17080` 管理面，不影响 `17081` JSON-RPC Token校验。
- 密码、密码哈希、Session和 CSRF Token不得通过普通设置接口返回。

## 3. 功能开发范围

### 3.1 Router职责拆分

目标文件：

- `server/src/api/mod.rs`
- `server/src/api/jsonrpc/mod.rs`
- 新增独立的管理 Router与 RPC Router构造模块或函数

执行内容：

1. 将当前单一 `router()` 拆成：
   - `management_router(state)`
   - `jsonrpc_router(state)`
2. `management_router` 注册 Web UI、`/api/*` 与 SSE，不合并 `jsonrpc::routes()`。
3. `jsonrpc_router` 只注册精确 `/jsonrpc`，不配置静态文件 fallback。
4. RPC Router未知路径统一返回 `404 Not Found`，不回退到 `index.html`。
5. CORS Header只保留在 RPC Router需要的 `/jsonrpc` 响应中，不扩大到管理 API。

完成标准：对 RPC Router请求 `/`、`/api/settings`、`/api/tasks`、`/api/events` 和任意静态资源均为 404。

### 3.2 双监听器运行模型

目标文件：

- `server/src/app/mod.rs`
- `server/src/main.rs`（如需命令行恢复入口）
- 对应独立 Rust测试文件

新增运行参数：

| 环境变量 | FPK默认值 | 说明 |
| --- | --- | --- |
| `MOTRIX_FNOS_HTTP_ADDR` | `0.0.0.0:17080` | 管理监听器 |
| `MOTRIX_FNOS_JSONRPC_ADDR` | `127.0.0.1:17081` | RPC专用监听器 |

执行内容：

1. `ServerRuntimeConfig` 增加 `jsonrpc_addr`。
2. 启动时分别绑定两个 `TcpListener`。
3. 两个 listener共享同一个 `HttpAppState`、SQLite连接、Aria2运行态和退出信号。
4. 任一 listener绑定失败时，server启动失败并输出明确的监听地址和错误原因，不进入半可用状态。
5. 收到退出信号时只执行一次 Aria2保存与清理流程，并等待两个 HTTP server停止。
6. 日志分别记录管理监听器和 RPC监听器地址，禁止记录 Token或密码。

完成标准：同一进程稳定提供两个监听器；停止、重启和异常退出不会重复清理 Aria2或遗留监听进程。

### 3.3 Web鉴权持久化模型

目标目录：

- `server/src/auth/`
- `server/src/database/`
- `server/src/settings/`（只负责设置页编排，不保存明文密码）

建议数据边界：

```text
web_auth_config
├── enabled
├── password_hash
├── password_updated_at
└── auth_version
```

执行内容：

1. 建立独立的 Web鉴权领域，不把密码字段塞入现有下载设置 `AppConfig`。
2. 使用 Argon2id和随机 salt保存密码哈希；数据库、日志和 API中不得出现明文密码。
3. 增加数据库迁移或独立设置 key；首次升级后进入待初始化状态，但保留所有任务、下载设置和 JSON-RPC Token。
4. 密码修改、关闭保护和本地重置时递增 `auth_version`，使旧 Session立即失效。
5. 数据库读取失败时安全失败：拒绝管理访问，不得自动降级为匿名模式。

完成标准：数据库中只能看到不可逆哈希；删除或损坏鉴权配置不会意外开放管理面。

### 3.4 Session与 CSRF

目标目录：

- `server/src/auth/session/`
- `server/src/api/auth/`
- 管理 Router认证中间件

执行内容：

1. 登录成功后签发随机、高熵、不可预测的 Session ID。
2. Session由服务端保存，Cookie只保存不透明 ID；server重启后允许 Session失效并要求重新登录。
3. Cookie属性：
   - `HttpOnly`
   - `SameSite=Strict`或经过验证的 `Lax`
   - 限定 `Path=/`
   - HTTPS请求设置 `Secure`；本地 HTTP入口保持可用但在 UI中标记非加密连接
4. 设置固定最长有效期和空闲超时，不提供永久 Session。
5. 修改密码、关闭保护、执行本地重置时清除全部 Session。
6. 对 `POST`、`PUT`、`DELETE` 等管理写操作校验 CSRF Token；不得仅依赖前端隐藏按钮。
7. SSE订阅必须验证 Session；失效后返回 401，前端停止无限重连并回到登录页。

完成标准：复制 Cookie、跨站表单或旧 Session不能绕过密码变更与 CSRF校验。

### 3.5 鉴权 API

新增接口建议：

| 方法 | 路径 | 是否需要 Session | 作用 |
| --- | --- | --- | --- |
| `GET` | `/api/auth/status` | 否 | 返回 `setupRequired`、`enabled`、当前会话状态 |
| `POST` | `/api/auth/setup` | 否，仅首次 | 初始化管理密码；已有密码后永久拒绝 |
| `POST` | `/api/auth/login` | 否 | 验证密码并创建 Session |
| `POST` | `/api/auth/logout` | 是 | 撤销当前 Session |
| `PUT` | `/api/auth/password` | 是，需当前密码 | 修改密码并撤销其他 Session |
| `PUT` | `/api/auth/protection` | 是，需当前密码 | 启用或关闭 Web管理保护 |

执行约束：

- `setup` 只能在数据库确认从未初始化时调用，不能通过并发请求重复初始化。
- 登录、修改密码和关闭保护使用统一密码校验与审计日志。
- 登录失败采用账号级或全局限速与递增延迟；不能信任未经验证的 `X-Forwarded-For`。
- 错误响应不得区分“密码不存在”和“密码错误”等可用于探测内部状态的信息。
- 普通 `/api/settings` 不再返回 JSON-RPC Token原文；如设置页面仍需修改 Token，应使用专门的受保护写接口和掩码读取模型。

完成标准：除首次初始化和登录外，所有鉴权配置变更都需要有效 Session、CSRF和当前密码。

### 3.6 管理 Router认证覆盖

必须保护：

```text
/api/tasks*
/api/settings*
/api/storage*
/api/aria2*
/api/debug-logs*
/api/events
其他新增管理 API
```

允许匿名加载但不得泄露业务数据：

```text
Web静态资源
/api/auth/status
/api/auth/setup（仅待初始化）
/api/auth/login
```

执行内容：

1. 管理 Router在进入业务 handler前统一验证 Session。
2. 未登录 API返回结构化 `401 Unauthorized`，不返回 SPA HTML。
3. 未登录访问根页面仍可加载登录/初始化界面。
4. 设置、任务、授权目录、日志和 JSON-RPC Token不得出现在未登录响应或首屏内嵌数据中。
5. 关闭 Web保护时由中间件明确进入匿名管理模式，并持续在 UI显示高风险警告。

完成标准：绕过前端直接调用任一敏感 API都会返回 401。

### 3.7 前端登录与初始化流程

目标目录：

```text
src/features/auth/
  components/
  composables/
  services/
  stores/
src/app/providers/
src/views/
```

执行内容：

1. 新增全局 Auth Store，负责初始化状态、登录、退出、Session失效和 CSRF Token。
2. 在进入 `MainWindow` 前增加访问门：
   - `setupRequired=true`：显示首次密码设置页
   - 未登录：显示登录页
   - 已登录或保护关闭：进入主窗口
3. HTTP client统一处理 401，清理前端业务状态并回到登录页。
4. SSE收到 401或会话失效后停止重连，登录成功后重新订阅。
5. 设置页新增“Web管理访问保护”：
   - 查看当前状态
   - 修改密码
   - 关闭/重新启用保护
   - 显示关闭保护的风险说明
6. 登录页不得展示任务名称、保存路径、版本更新详情或诊断信息。
7. 补充中文、英文文案以及移动端键盘、密码管理器、回车提交和错误状态。

完成标准：刷新、Session超时、server重启和多标签页场景均能稳定回到登录状态，不出现短暂业务数据闪现。

### 3.8 本地密码恢复

目标文件：

- server命令行入口或专用恢复模块
- `packaging/fnos/cmd/reset-web-auth`
- `scripts/tests/` 中的脚本测试

执行内容：

1. 提供只能在 NAS本机/SSH执行的恢复命令，不提供隐藏公网重置 URL。
2. 恢复工具清除 Web密码哈希和 Session，重新进入首次初始化状态。
3. 恢复工具不得修改：
   - 下载任务
   - Aria2 session
   - 下载设置
   - JSON-RPC Token
   - 授权目录
4. 明确要求停止应用或使用安全数据库事务，避免与运行中 server并发写入。
5. 输出脱敏结果并写入本地安全日志。

完成标准：忘记密码时可通过 SSH恢复，同时不存在远程匿名重置路径。

### 3.9 FPK与 Lucky接入

目标文件：

- `packaging/fnos/cmd/common.sh`
- `packaging/fnos/cmd/start`
- `packaging/fnos/cmd/status`
- `packaging/fnos/cmd/stop`
- `packaging/fnos/MotrixFNOS.sc`
- `scripts/build-fpk.mjs`
- `scripts/tests/`

执行内容：

1. FPK启动时注入 `MOTRIX_FNOS_JSONRPC_ADDR=127.0.0.1:17081`。
2. `MotrixFNOS.sc` 只声明 `17080` 管理端口，不声明、不转发 `17081`。
3. FPK预检验证：
   - 管理端口仍与 manifest `service_port` 一致
   - RPC地址必须是回环地址
   - RPC端口不得出现在 fnOS `port-config`
4. `status` 继续验证主进程身份，不以单个端口是否可用替代进程校验。
5. Lucky最终配置：

   ```text
   外部：https://motrix.andy-singapore.ccwu.cc:8443/jsonrpc
   后端：http://127.0.0.1:17081/jsonrpc
   ```

6. Lucky默认规则和非 `/jsonrpc` 路径返回 403/404；Cloudflare WAF继续作为额外防线。

完成标准：从 NAS局域网无法直接连接 `17081`；通过 Lucky访问根路径和 `/api/*`失败，只有 `/jsonrpc`成功。

## 4. 实施顺序

### 阶段 0：立即收口现有公网入口

在代码完成前先执行运维措施：

1. Lucky或 Cloudflare只允许精确 `/jsonrpc`。
2. 禁止公网 `/` 与 `/api/*`。
3. 轮换当前 JSON-RPC Token。
4. 验证 `/api/settings`、`/api/tasks` 和根页面均返回 403/404。

该阶段不替代代码改造，只用于消除当前暴露窗口。

### 阶段 1：先更新长期文档

在任何代码修改前更新：

1. `docs/architecture.md`：将单监听器改为管理/RPC双监听器，明确认证边界。
2. `docs/api-contract.md`：定义认证 API、401、Session、CSRF、17081与 `/jsonrpc` 唯一入口。
3. `docs/development-plan.md`：将安全访问改造列为 P0，并明确阻塞公网正式使用。

文档获批后进入代码实施。

### 阶段 2：Router隔离与双监听器

1. 拆分 Router。
2. 增加 `jsonrpc_addr` 配置。
3. 启动两个 listener并统一退出。
4. 完成路由隔离和生命周期测试。

验收后，Lucky可以先切到 `17081`，即使 Web密码尚未完成，公网也已经无法访问管理面。

### 阶段 3：服务端鉴权核心

1. 数据库迁移与 Argon2id密码模型。
2. Session、Cookie、CSRF和限速。
3. 鉴权 API。
4. 管理 Router中间件覆盖。
5. 本地恢复命令。

完成后使用纯 HTTP/API测试验证，暂不依赖前端。

### 阶段 4：前端访问门与设置

1. Auth Store与 service。
2. 首次初始化页。
3. 登录/退出流程。
4. 401与 SSE失效处理。
5. 设置页密码修改和保护开关。
6. 中英文与响应式验证。

### 阶段 5：FPK、Lucky与实机验证

1. 更新 FPK环境变量和预检。
2. 构建 x86/ARM FPK。
3. fnOS实机升级并保留任务数据。
4. Lucky切换到 `17081`。
5. IPv4-only与 IPv6外网分别验证 JSON-RPC。
6. 验证 fnOS桌面、局域网和 FN Connect管理入口的登录流程。

### 阶段 6：清理旧约定与发布

1. 删除所有关于 `17080/jsonrpc` 的实现、测试和文档。
2. 更新远程接入备忘录与排障说明。
3. 完整执行发布验证。
4. 在 CHANGELOG明确记录不兼容变更：JSON-RPC迁移到回环 `17081`，Web管理首次使用必须设置密码。

## 5. 测试计划

### 5.1 Rust自动化测试

- 管理 Router不存在 `/jsonrpc`。
- RPC Router除 `/jsonrpc` 外全部 404。
- `17081`默认解析为 `127.0.0.1:17081`，拒绝非法或非回环 FPK配置。
- 两个 listener任一绑定失败时启动失败。
- 退出清理只执行一次。
- 首次初始化只能成功一次，并发初始化只有一个成功。
- 密码哈希验证、错误密码、修改密码和关闭保护。
- Session创建、过期、撤销、密码变更失效。
- CSRF缺失或错误时拒绝管理写操作。
- 未登录访问所有敏感 API返回 401。
- 已登录任务、设置、SSE和诊断流程保持可用。
- 鉴权数据库失败时不会降级匿名访问。
- JSON-RPC Token校验与现有 multicall测试保持通过。

### 5.2 前端自动化测试

- 首次启动显示初始化页，不挂载 `MainWindow`。
- 初始化成功进入主窗口。
- 登录失败、限速提示和登录成功。
- 401统一回到登录页并清空敏感 store。
- SSE会话失效停止重连。
- 修改密码后其他 Session失效。
- 关闭保护需要当前密码和二次确认。
- 登录页不渲染任务、路径和 Token。

### 5.3 构建与脚本测试

- FPK预检拒绝把 `17081`加入端口转发。
- 启动脚本正确注入两个地址。
- 本地恢复脚本只修改 Web鉴权状态。
- `pnpm run verify:pre-commit` 每批通过。
- 最终 `pnpm run verify` 通过。

### 5.4 实机矩阵

| 场景 | 预期 |
| --- | --- |
| fnOS桌面首次进入 | 必须设置 Web管理密码 |
| 局域网访问 `http://NAS:17080` | 未登录只显示登录页，API为401 |
| 外网访问 `https://motrix...:8443/` | 403/404 |
| 外网访问 `https://motrix...:8443/api/settings` | 403/404，响应不含 Token |
| HTTP JSON-RPC `POST /jsonrpc` | 正确 Token可添加任务 |
| WebSocket `GET /jsonrpc` | 可升级并执行受支持方法 |
| NAS局域网访问 `NAS:17081` | 连接失败 |
| server重启 | Web Session失效，任务与设置保留 |
| 忘记密码后 SSH重置 | 回到首次初始化，任务和 RPC Token保留 |
| IPv4-only公司网络 | Cloudflare/Lucky可提交任务，不能打开管理面 |
| IPv6外网 | fnOS/FN Connect管理路径与 RPC边界互不混用 |

## 6. 必须同步更新的文档

### 实施前更新

- `docs/architecture.md`
  - 单监听器改为双监听器。
  - 定义管理面、RPC面、Web Session和 JSON-RPC Token边界。
  - 删除“Web UI、API、SSE与 JSON-RPC共用同一监听器”的长期约束。

- `docs/api-contract.md`
  - 新增 `MOTRIX_FNOS_JSONRPC_ADDR`。
  - 删除 `17080/jsonrpc`。
  - 新增 `/api/auth/*` 契约、401、Cookie、Session、CSRF和鉴权状态模型。
  - 明确管理 API必须鉴权，RPC监听器只提供 `/jsonrpc`。

- `docs/development-plan.md`
  - 新增 P0安全访问阶段。
  - 明确本计划优先于公网接入和非阻塞 UI视觉工作。

### 实施期间更新

- `docs/design/ui-product-requirements.md`
  - 增加首次初始化、登录、Session失效、修改密码和关闭保护确认状态。

- `docs/design/DESIGN.md`
  - 增加登录/初始化页的视觉、响应式、错误、loading和密码输入规则。

- `docs/fpk-packaging.md`
  - 增加双监听器环境变量、回环端口约束、FPK预检和实机检查命令。
  - 明确 `17081` 不进入 manifest `service_port`、`MotrixFNOS.sc` 或 fnOS端口映射。

- `docs/jsonrpc-remote-access.md`
  - Lucky后端改为 `127.0.0.1:17081`。
  - 删除公网访问 Motrix页面和 `/api/*` 的测试步骤。
  - 新增根路径/API必须拒绝、只有 `/jsonrpc`成功的验证矩阵。

### 发布时更新

- `CHANGELOG.md`
  - 记录双监听器、Web管理密码、RPC端口迁移和不兼容变化。

- `README.md`（如果仍包含旧端口或远程访问说明）
  - 更新管理端口与 JSON-RPC入口说明。

## 7. 明确不做

- 不通过来源 IP判断公网、局域网或 FN Connect。
- 不信任客户端可伪造的 `Host`、`X-Forwarded-For` 或任意用户 Header。
- 不把 Web密码复用为 JSON-RPC Token。
- 不把 JSON-RPC Token作为 Web管理 Session。
- 不在前端保存明文密码、密码哈希或长期 Session Token。
- 不提供公网密码重置 URL。
- 不在 `17081` 提供静态文件、健康检查、设置或任务 REST API。
- 不保留 `17080/jsonrpc` 兼容入口。
- 本阶段不同时实施 fnOS统一网关迁移；统一网关继续按独立最小 FPK验证门禁推进。

## 8. 完成定义

以下条件全部满足才算完成：

1. `17080` 不存在 `/jsonrpc`，管理 API默认需要 Web Session。
2. `17081` 仅绑定回环地址且只有 `/jsonrpc`。
3. 公网根页面和 `/api/*` 无法通过 Lucky/Cloudflare访问。
4. 正确 JSON-RPC Token可通过 IPv4-only网络添加任务。
5. 首次初始化、登录、退出、修改密码、关闭保护和 SSH恢复流程完整。
6. 密码哈希、Session、CSRF、限速和安全日志通过自动化测试。
7. x86/ARM FPK预检、构建和 fnOS实机升级验证通过。
8. 所有必要文档与 CHANGELOG已同步，不再存在单监听器或 `17080/jsonrpc` 的旧说明。

