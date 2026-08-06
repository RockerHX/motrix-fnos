# 前后端 HTTP / SSE / JSON-RPC API 契约

> 本文档定义 Rust server、Vue Web UI 与外部 JSON-RPC 兼容调用方之间的接口边界。总体架构见 `docs/architecture.md`；开发计划与后续候选事项见 `docs/future-development-plan.md`；UI 产品需求见 `docs/design/ui-product-requirements.md`；FPK 构建与产物见 `docs/fpk-packaging.md`。

## 1. 运行时约定

| 环境变量 | 作用 | 默认值 |
| --- | --- | --- |
| `MOTRIX_FNOS_APP_DATA_DIR` | server 数据目录 | 用户本地数据目录下的 `motrix-fnos` |
| `MOTRIX_FNOS_HTTP_ADDR` | 管理监听地址 | `0.0.0.0:17080` |
| `MOTRIX_FNOS_JSONRPC_ADDR` | JSON-RPC 专用监听地址 | `127.0.0.1:17081` |
| `MOTRIX_FNOS_LAN_JSONRPC_ADDR` | 局域网 JSON-RPC 监听地址 | `0.0.0.0:17082` |
| `MOTRIX_FNOS_ARIA2_PATH` | Aria2 可执行文件路径 | 打包路径优先，仓库调试路径兜底 |
| `MOTRIX_FNOS_ACCESSIBLE_PATHS_FILE` | fnOS 已授权目录快照文件 | `MOTRIX_FNOS_APP_DATA_DIR/accessible-paths.json` |
| `MOTRIX_TRUSTED_PROXY_IPS` | 可信反向代理的直接对端 IP，逗号分隔 | 空，不读取代理来源 Header |
| `MOTRIX_WEB_COOKIE_SECURE` | 是否为 Web Session Cookie 添加 `Secure` | `false` |

FPK 脚本从 fnOS 注入的 `TRIM_DATA_ACCESSIBLE_PATHS` 读取已授权目录，并写入 `MOTRIX_FNOS_ACCESSIBLE_PATHS_FILE`。后端以该文件为主，文件不存在时才回退读取当前进程环境变量。

监听器约定：

- 管理监听器只承载 Web UI、`/api/*` 与 `/api/events`，未知路径统一返回 `404 Not Found`。
- 回环 JSON-RPC 监听器只绑定回环地址，只注册精确的 `GET`、`POST` 和 `OPTIONS /jsonrpc`；其他路径统一返回 `404 Not Found`，不配置 SPA fallback。
- 局域网 JSON-RPC 监听器始终绑定 IPv4 `17082`；入口关闭时精确路径也返回 `404`，开启后只接受 RFC1918 IPv4 真实对端，其他来源返回 `403`。该判断不得读取 `X-Forwarded-For`。
- `MOTRIX_FNOS_JSONRPC_ADDR` 在 FPK 中必须解析为回环地址，且不得进入 manifest、`MotrixFNOS.sc` 或 fnOS 端口映射。
- `MOTRIX_FNOS_LAN_JSONRPC_ADDR` 在 FPK 中固定为 `0.0.0.0:17082`，通过 `MotrixFNOS.sc` 与管理端口共同声明，但不得成为 manifest 或桌面入口端口。
- 三个监听器共享业务状态和退出信号；任一地址绑定失败时 server 整体启动失败。

## 2. 前端消费约定

- FPK Web UI 从 manifest `service_port` 对应的管理端口访问后端，API 与 SSE 使用同源 `/api/*` 和 `/api/events`。
- FPK 桌面入口与 Rust server 使用同一端口；不得再同时声明统一网关字段并把该端口限制成仅 JSON-RPC。
- 浏览器请求使用同源服务端 Session Cookie；管理写操作通过 `X-CSRF-Token` 请求头携带 CSRF Token。
- `/jsonrpc` 只存在于两个 RPC listener，不与 Web UI 共用监听器；回环反代和局域网写操作分别要求独立 Token。
- 开发态由 Vite proxy 转发 `/api` 与 `/api/events` 到本地 server。
- JSON 接口使用浏览器原生 `fetch`。
- SSE 使用浏览器原生 `EventSource`。
- 错误提示优先展示响应体中的 `message`；响应体不符合统一错误结构时按 HTTP 状态码展示通用错误。
- 下载保存目录只允许从后端返回的 fnOS 已授权目录中选择；Web UI 不提供任意本地路径选择器。

## 3. 错误响应

由业务 handler 返回的错误使用统一 JSON 响应：

```json
{
  "code": "task_conflict",
  "message": "应用正在退出，不能执行任务操作"
}
```

状态码约定：

| 状态码 | 含义 |
| --- | --- |
| `400 Bad Request` | 请求参数非法或业务校验失败 |
| `401 Unauthorized` | 管理 Session 缺失、无效或过期；不得返回 SPA HTML |
| `403 Forbidden` | CSRF Token 缺失或错误，或当前凭据无权执行操作 |
| `404 Not Found` | `/api` 下不存在的路径；可能不带 JSON body |
| `408 Request Timeout` | 请求在资源限制层超时；可能不带 JSON body |
| `409 Conflict` | 当前运行状态不允许执行该操作 |
| `413 Payload Too Large` | 请求体超过接口大小限制；可能不带 JSON body |
| `429 Too Many Requests` | 登录失败限速或递增延迟生效 |
| `502 Bad Gateway` | Aria2 明确拒绝任务代理等运行选项 |
| `503 Service Unavailable` | Aria2 生命周期转换或运行依赖暂时不可用 |
| `500 Internal Server Error` | 未预期内部错误 |

`204 No Content` 响应不带 JSON body。

`404`、`408` 与 `413` 由路由或资源限制层在进入业务 handler 前生成，不保证返回 `{ "code", "message" }`。调用方必须按 HTTP 状态码处理这三类响应。

## 4. HTTP API

下表使用同源 `/api` 路径，FPK 与开发态保持一致。

### 4.1 Web 管理鉴权

静态资源允许匿名加载，但不得包含任务、路径、设置、日志或 Token 等业务数据。除本节明确标记为匿名的接口外，所有 `/api/*` 与 `/api/events` 默认要求有效 Session；保护关闭时，普通管理 API 可进入显式匿名管理模式，鉴权配置变更仍按本节要求校验当前密码。

| 方法 | 路径 | 访问要求 | 作用 |
| --- | --- | --- | --- |
| `GET` | `/api/auth/status` | 匿名 | 返回初始化、保护和当前会话状态 |
| `POST` | `/api/auth/setup` | 匿名，仅从未初始化时 | 初始化 Web 管理密码并创建 Session |
| `POST` | `/api/auth/login` | 匿名 | 验证密码并创建 Session |
| `POST` | `/api/auth/logout` | Session + CSRF | 撤销当前 Session |
| `PUT` | `/api/auth/password` | Session + CSRF + 当前密码 | 修改密码并撤销其他 Session |
| `PUT` | `/api/auth/protection` | Session + CSRF + 当前密码 | 启用或关闭 Web 管理保护 |

`GET /api/auth/status` 响应：

```json
{
  "setupRequired": false,
  "enabled": true,
  "authenticated": true,
  "csrfToken": "opaque-csrf-token"
}
```

约定：

- `setupRequired=true` 时，除 `GET /api/auth/status`、`POST /api/auth/setup`、`GET /api/app/ready` 和静态资源外，管理 API 均返回 `401 Unauthorized`。
- `csrfToken` 只在当前浏览器具备管理访问权时返回；未登录且保护已启用时为 `null`。保护关闭时，服务端可为匿名管理浏览器签发短期访问上下文并返回对应 CSRF Token。
- 前端不得把 CSRF Token 写入持久存储；Session 失效后必须丢弃旧 Token。
- 鉴权配置读取失败时 `status` 安全失败，不得把 `enabled` 自动降级为 `false`。

`POST /api/auth/setup` 与 `POST /api/auth/login` 请求：

```json
{
  "password": "user-entered-password"
}
```

- `setup` 必须在数据库事务中确认从未初始化；并发初始化最多一个请求成功，其余返回 `409 Conflict`。
- `login` 失败统一返回相同的 `401` 错误，不区分密码不存在、密码错误或内部状态；连续失败返回 `429` 或施加递增延迟。
- 登录限速默认使用管理 listener 的真实对端 IP。只有对端 IP 命中 `MOTRIX_TRUSTED_PROXY_IPS` 时，才使用 `X-Forwarded-For` 中第一个合法 IP；直连、未配置或未命中的代理都忽略该 Header。

`PUT /api/auth/password` 请求：

```json
{
  "currentPassword": "current-password",
  "newPassword": "new-password"
}
```

`PUT /api/auth/protection` 请求：

```json
{
  "enabled": false,
  "currentPassword": "current-password"
}
```

- 密码修改、保护状态变更与本地重置必须递增 `authVersion` 并撤销已有 Session；修改密码成功时可只为当前请求签发新 Session。
- 关闭保护不会删除密码哈希，也不会更改 JSON-RPC Token；重新启用保护仍需当前密码。
- 密码明文、密码哈希、Session ID、Cookie 与 CSRF Token 不得写入日志、普通设置响应或调试日志。

Session 与 Cookie 约定：

- 密码使用 Argon2id 和随机 salt 保存不可逆哈希；Session ID 使用密码学安全随机源生成，仅在服务端内存保存。
- Cookie 名固定为 `motrix_web_session`，只保存不透明 Session ID，并设置 `HttpOnly`、`SameSite=Strict`、`Path=/` 和固定最长有效期；`MOTRIX_WEB_COOKIE_SECURE=false` 时不带 `Secure`，显式设为 `true` 时登录、密码变更、退出清除等 Cookie 都带 `Secure`。server 不根据客户端可伪造的代理 Header 自动切换该属性。
- Session 同时受固定最长有效期和空闲超时限制；server 重启后允许全部失效，不提供永久 Session。
- `POST`、`PUT`、`PATCH`、`DELETE` 等管理写操作必须校验与当前访问上下文绑定的 `X-CSRF-Token`，缺失或错误时返回结构化 `403 Forbidden`。
- `/api/events` 必须校验 Session 或已关闭保护的匿名访问上下文；失效时返回 `401`，前端停止无限重连并回到登录页。

统一鉴权错误示例：

```json
{
  "code": "authentication_required",
  "message": "请先登录 Web 管理界面"
}
```

### 4.2 应用信息

| 方法 | 路径 | 响应 |
| --- | --- | --- |
| `GET` | `/api/app/info` | `AppInfo` |
| `GET` | `/api/app/ping` | `BackendPing` |
| `GET` | `/api/app/ready` | `AppReadiness` |
| `GET` | `/api/app/update-check` | `AppUpdateCheck` |

`AppInfo` 示例：

```json
{
  "name": "Motrix",
  "version": "<currentVersion>",
  "backendStatus": "ready",
  "maintainer": "rockerhx",
  "repositoryUrl": "https://github.com/RockerHX/motrix-fnos",
  "releasePageUrl": "https://github.com/RockerHX/motrix-fnos/releases",
  "targetArch": "x86_64",
  "updateMode": "manual_fpk_or_app_center"
}
```

`AppReadiness` 就绪响应：

```json
{
  "ready": true
}
```

约定：

- `/api/app/ready` 仅用于 Rust 服务生命周期探测，无需管理 Session。
- 只有管理与 JSON-RPC listener 已绑定、启动门禁已经完成且服务未进入退出状态时返回 `200 OK`；其他状态返回 `503 Service Unavailable` 和 `app_not_ready`。
- SQLite 初始化仍是 listener 标记就绪前的启动门禁，但 ready 请求本身只读取内存状态，不获取数据库连接、不执行 SQL、不写日志。

`AppUpdateCheck` 示例：

```json
{
  "currentVersion": "<currentVersion>",
  "latestVersion": "<latestVersion>",
  "hasUpdate": true,
  "status": "available",
  "releaseUrl": "https://github.com/RockerHX/motrix-fnos/releases/tag/v1.3.4",
  "assets": [
    {
      "architecture": "x86",
      "name": "motrix_1.3.4_x86.fpk",
      "downloadUrl": "https://github.com/RockerHX/motrix-fnos/releases/download/v1.3.4/motrix_1.3.4_x86.fpk"
    },
    {
      "architecture": "arm",
      "name": "motrix_1.3.4_arm.fpk",
      "downloadUrl": "https://github.com/RockerHX/motrix-fnos/releases/download/v1.3.4/motrix_1.3.4_arm.fpk"
    }
  ],
  "checkedAt": 1760000000000,
  "message": "检测到新版本，请下载匹配设备架构的 FPK 后在 fnOS 应用中心手动安装。"
}
```

约定：

- `updateMode` 当前固定为 `manual_fpk_or_app_center`，表示应用内只提供版本检测和更新提示，不执行 FPK 自动安装。
- `targetArch` 使用运行中 server 的 CPU 架构，用于前端提示用户选择 x86 或 ARM 包。
- `GET /api/app/update-check` 由后端请求 GitHub Releases latest；网络失败或解析失败时仍返回 `200 OK`，`status` 为 `unavailable`，并给出可展示 `message`。
- `assets` 只识别 `*_x86.fpk` 与 `*_arm.fpk`；无法识别的附件不返回给前端。

### 4.3 Aria2

| 方法 | 路径 | 说明 | 响应 |
| --- | --- | --- | --- |
| `GET` | `/api/aria2/config` | 读取 Aria2 配置状态 | `Aria2ConfigStatus` |
| `GET` | `/api/aria2/process` | 读取受管进程状态 | `Aria2ProcessStatus` |
| `GET` | `/api/aria2/rpc` | 读取 RPC 连通状态 | `Aria2RpcStatus` |
| `POST` | `/api/aria2/start` | 启动受管 Aria2 | `Aria2ProcessStatus` |
| `POST` | `/api/aria2/stop` | 停止受管 Aria2 | `Aria2ProcessStatus` |

上述读取接口不得因为查询启动 Aria2。`/api/aria2/rpc` 在 Aria2 停止时直接返回 `connected=false` 的停止态，不发起 RPC 探测。
需要 Aria2 的任务操作、外部 `aria2.addUri`、启动恢复和后台监控统一经过生命周期协调器；不需要引擎的回收站永久删除等操作保持独立。
`POST /api/aria2/stop` 遇到活动任务、metadata、在途操作或排队请求时返回 `409 Conflict`，错误码为 `aria2_busy`，不隐式暂停任务。空闲停止先保存必要状态和 session，确认进程退出后才清除运行态；保存或停止失败时保留运行态并返回 `503 Service Unavailable`，错误码为 `aria2_stop_failed`，客户端可以重试。

### 4.4 任务

| 方法 | 路径 | 请求 | 响应 |
| --- | --- | --- | --- |
| `GET` | `/api/tasks` | - | `DownloadTask[]` |
| `GET` | `/api/tasks?status=removed` | - | `DownloadTask[]` |
| `POST` | `/api/tasks` | `CreateDownloadTaskRequest` | `DownloadTask` |
| `POST` | `/api/tasks/batch` | `CreateBatchDownloadTasksRequest` | `CreateBatchDownloadTasksResponse` |
| `POST` | `/api/tasks/torrent` | `multipart/form-data` | `DownloadTask` |
| `POST` | `/api/tasks/:id/confirm` | `ConfirmTaskFilesRequest` | `DownloadTask` |
| `POST` | `/api/tasks/:id/pause` | - | `DownloadTask` |
| `POST` | `/api/tasks/:id/resume` | - | `DownloadTask` |
| `POST` | `/api/tasks/:id/redownload` | 可选 `TaskProxyOverrideRequest` | `DownloadTask` |
| `POST` | `/api/tasks/:id/restore` | 可选 `TaskProxyOverrideRequest` | `DownloadTask` |
| `PUT` | `/api/tasks/:id/proxy` | `UpdateTaskProxyRequest` | `DownloadTask` |
| `DELETE` | `/api/tasks/:id?deleteFiles=true|false` | - | `DownloadTask` |
| `DELETE` | `/api/tasks/:id/permanent` | - | `204 No Content` |

约定：

- `GET /api/tasks` 只返回未删除任务。
- 普通 `GET /api/tasks` 和 WebUI 普通刷新只返回内存任务快照，不启动 Aria2 或执行无变化完整持久化；活动任务由后台监控或明确任务操作刷新，不新增独立同步 API。
- `GET /api/tasks?status=removed` 只返回已删除任务记录，用于回收站页面。
- `status` 当前只支持 `removed`；其他值返回 `400 Bad Request`。
- `POST /api/tasks/:id/restore` 只允许恢复 `removed` 任务；恢复成功后任务进入暂停状态，不会立即占用下载带宽。
- `POST /api/tasks/:id/redownload` 只允许重新下载 `complete` 任务。服务端先按原来源创建暂停任务并持久化，再暂存旧文件；新任务恢复成功后才清理暂存文件。任一步失败会恢复旧任务和原文件。URL 使用 `addUri`，种子和已确认磁链使用保存的源 metadata 调用 `addTorrent`。
- 恢复与重新下载可省略请求体；省略或请求体中不提供 `useProxy` 时继承原任务的完整代理绑定。显式改变开关时改用应用代理配置来源；显式值与原值相同则保留旧兼容绑定。
- 恢复与重新下载必须在创建目录、提交 GID 或暂存旧文件前确认所继承或覆盖的代理可用。校验或应用失败时保持原任务、文件和回收站状态，不得回退为直连。
- 同一任务已有重新下载操作时，重复请求返回 `409 Conflict`，错误码为 `task_operation_conflict`。
- 恢复保留本地文件的任务时复用原保存目录和控制文件续传；删除过本地文件的任务重建为从头下载的暂停任务。
- URL 任务使用原 URL 恢复；种子和已确认磁链优先使用应用私有目录保存的源种子 metadata。磁链缺少 metadata 时重新解析并再次要求确认文件；旧种子 metadata 已丢失时返回 `400 Bad Request` 并保持回收站状态。
- `DELETE /api/tasks/:id/permanent` 只允许永久删除已删除任务记录；该操作只清理 Motrix 数据库记录，不删除用户下载文件。
- 永久删除同时清理该任务位于应用私有目录的恢复 metadata；该清理不属于用户下载文件删除。
- 磁力链接会先由 Aria2 下载 metadata；metadata 完成后任务会跟随到真实 BT GID，状态保持 `paused`，并设置 `confirmationRequired=true`。前端必须展示 `files` 让用户确认后再调用 `/api/tasks/:id/confirm` 开始真实下载。
- 磁力链接任务会在用户授权的父保存目录下创建任务专属子目录，并启用 Aria2 `bt-save-metadata=true`；解析出的 hash 命名 `.torrent` 会和下载产物、`.aria2` 控制文件一起放在该目录。该 `.torrent` 仅作为磁链解析过程产物用于可见性 / 排障，不替代 Aria2 session 机制。
- 当 `confirmationRequired=true` 时，普通 `/api/tasks/:id/resume` 会返回 `400 Bad Request`，提示先确认要下载的文件，避免绕过文件选择。

`CreateDownloadTaskRequest`：

```json
{
  "url": "https://example.com/file.zip",
  "fileName": "file.zip",
  "saveDir": "/vol1/downloads",
  "sourceType": "url",
  "startMode": "now",
  "category": "默认",
  "advancedOptions": {
    "connections": 8,
    "downloadLimitKb": 0,
    "useProxy": false
  }
}
```

约定：

- `sourceType` 可选值为 `url` / `magnet`；种子文件通过 `/api/tasks/torrent` 上传，不在 JSON 请求中传 `torrent`。省略时兼容旧请求并按 `url` 处理。
- `startMode` 可选值为 `now` / `paused`，省略时按 `now` 处理。
- `category` 是 Motrix 任务标签，默认 `默认`；它不改变保存目录，也不影响侧栏状态分类。
- `advancedOptions.connections` 映射 Aria2 `split` 与 `max-connection-per-server`；`advancedOptions.downloadLimitKb` 映射单任务下载限速。
- `advancedOptions.useProxy` 是可选布尔值，缺失时服务端按 `false` 处理。值为 `true` 时任务绑定当前应用代理配置；配置不存在时返回 `400 proxy_not_configured`，并且不创建任务、目录或 Aria2 GID。
- 旧 `advancedOptions.proxy` 继续兼容：未同时提供 `useProxy` 时，非空原始值作为该任务的私密兼容覆盖并映射 `all-proxy`。同时提供 `useProxy` 与非空 `advancedOptions.proxy` 时返回 `400 proxy_conflict`。
- `saveDir` 必须来自 `/api/storage/accessible-paths` 返回的 `paths`；为空或未授权路径会返回 `400 Bad Request`。
- 当 `sourceType=magnet` 时，请求中的 `saveDir` 表示授权父目录；成功创建后返回的 `DownloadTask.saveDir` 是后端创建的任务专属子目录。
- BT 任务返回的 `ownedTaskDir` 是后端创建并持久化的外层任务目录；它独立于会随种子 metadata 更新的 `fileName`，仅用于任务恢复和安全删除。普通 URL 任务为 `null`。
- `aria2Options` 为兼容字段；Web UI 不直接使用该字段，外部调用或 `/jsonrpc` 兼容入口可传入受支持的 Aria2 参数。
- `aria2Options["all-proxy"]` 遵循与旧 `advancedOptions.proxy` 相同的私密兼容规则；同时提供 `useProxy` 与非空原始代理字段时返回 `400 proxy_conflict`。两个旧原始字段同时存在时沿用既有白名单选项优先级，但只持久化最终生效值。
- 后端只透传白名单内的 Aria2 选项，并会覆盖 `dir` / `out`，确保保存目录和文件名仍由 Motrix 校验。

`DownloadTask`：

```json
{
  "id": 1,
  "url": "https://example.com/file.zip",
  "sourceType": "url",
  "fileName": "file.zip",
  "saveDir": "/vol1/downloads",
  "ownedTaskDir": null,
  "category": "默认",
  "gid": "abc123",
  "status": "pending",
  "totalLength": 1024,
  "completedLength": 0,
  "downloadSpeed": 0,
  "errorCode": null,
  "errorMessage": null,
  "filePath": "/vol1/downloads/file.zip",
  "useProxy": false,
  "confirmationRequired": false,
  "files": [],
  "createdAt": 1760000000000,
  "updatedAt": 1760000000000
}
```

`DownloadTaskFile`：

```json
{
  "index": 1,
  "path": "/vol1/downloads/movie/file-a.mkv",
  "name": "file-a.mkv",
  "length": 1024,
  "completedLength": 0,
  "selected": true
}
```

约定：

- `category` 是任务标签，默认 `默认`。
- `useProxy` 只表示任务的持久化代理意图；响应不返回代理来源、配置 revision、代理 URL 或兼容覆盖值。
- `confirmationRequired` 表示任务需要用户确认文件后才能继续下载。
- `files` 来自 Aria2 `tellStatus` 的运行时文件列表，不写入 SQLite；应用重启后通过 Aria2 session 同步重新填充。
- `DownloadTaskFile.index` 使用 Aria2 原生 one-based 文件索引，前端不得重排后再提交。

`CreateBatchDownloadTasksRequest`：

```json
{
  "urls": ["https://example.com/file-a.zip", "https://example.com/file-b.zip"],
  "saveDir": "/vol1/downloads",
  "startMode": "now",
  "category": "默认",
  "advancedOptions": {
    "connections": 8,
    "downloadLimitKb": 0,
    "useProxy": false
  }
}
```

`CreateBatchDownloadTasksResponse`：

```json
{
  "created": [],
  "failed": [
    {
      "input": "ftp://example.com/file.zip",
      "message": "当前仅支持 HTTP / HTTPS 下载链接"
    }
  ]
}
```

`POST /api/tasks/torrent` 使用 `multipart/form-data`：

- `torrent`：种子文件，大小不得超过 10 MiB。
- `request`：JSON 字符串，字段为 `saveDir`、`startMode`、`category`、`advancedOptions`。
- `request.advancedOptions.useProxy` 与 JSON 创建接口语义一致；缺失按 `false`。种子创建、磁链 metadata GID 与确认后的最终 BT GID 必须使用同一代理绑定。
- `request.saveDir` 表示用户授权的父保存目录；服务端会按种子任务名创建专属子目录，并将 Aria2 下载目录设为该子目录。
- 成功后返回创建出的 `DownloadTask`；`url` 存为 `torrent:<原始文件名>`，`saveDir` 为任务专属子目录。
- 服务端保留 Aria2 原生上传元数据落盘行为；任务专属子目录内可能出现 hash 命名 `.torrent` 文件，用于保持 Aria2 session 恢复语义，不再额外保存同名 `.torrent` 副本。

`ConfirmTaskFilesRequest`：

```json
{
  "selectedFileIndexes": [1, 3, 5]
}
```

约定：

- `selectedFileIndexes` 不能为空；后端会过滤非正数、去重并排序。
- 后端将选择结果映射为 Aria2 `changeOption(gid, { "select-file": "1,3,5" })`，随后调用 `aria2.unpause` 开始真实 BT 下载。
- 成功后返回更新后的 `DownloadTask`，其中 `confirmationRequired=false`，任务进入下载中状态。
- 本接口只负责确认文件并开始下载；磁链解析出的 `.torrent` 元数据由创建磁链任务时的 `bt-save-metadata` 选项触发保存。
- 服务端在 `unpause` 前对账任务持久化代理意图；代理无法解析或应用时保持暂停并返回结构化错误，不得直连。

`UpdateTaskProxyRequest`：

```json
{
  "enabled": true
}
```

`TaskProxyOverrideRequest`：

```json
{
  "useProxy": true
}
```

任务代理切换约定：

- 活动或已暂停且仍有有效 GID 的任务通过受控 `aria2.changeOption` 更新 `all-proxy`；运行中切换可能短暂重建连接，但不删除文件、不更换 Motrix 任务 ID。
- Aria2 已停止时只保存任务意图，不为切换操作启动引擎。完成和回收站任务也只保存供将来继承的状态。
- 开启应用代理而配置不存在时返回 `400 proxy_not_configured`。任务不存在返回 `404 task_not_found`；已有互斥操作返回 `409 task_operation_conflict`；生命周期转换返回 `409 runtime_transition`；Aria2 明确拒绝返回 `502 proxy_apply_failed`。
- 对有效 GID，服务端先应用 Aria2 option 再提交 SQLite；持久化失败时补偿旧 option。RPC 结果未知时保留旧 SQLite 事实及未完成操作，响应不得假装成功。
- 关闭旧兼容代理后删除私密覆盖并把来源转为应用配置；之后再次开启使用当前应用代理配置。

任务代理持久化约定：

- SQLite schema v4 为 `download_tasks` 增加 `use_proxy INTEGER NOT NULL DEFAULT 0` 与 `proxy_source TEXT NOT NULL DEFAULT 'profile'`。升级前任务统一迁移为关闭，其他任务字段保持不变，重复启动不得重复迁移。
- 私密表 `task_proxy_overrides(task_id, proxy_url, updated_at)` 只保存旧接口的任务专属代理，`task_id` 为主键并随任务永久删除清理。普通任务读写不得把 `proxy_url` 复制到公开字段、操作记录或日志。
- SQLite 任务意图是长期事实；Aria2 session 只用于恢复。启动、继续、stale GID 重建、磁链跟随 GID 和空闲重启都必须在 unpause 前按 SQLite 对账 `all-proxy`，失败时保持暂停或进入可诊断错误状态。

### 4.5 设置

| 方法 | 路径 | 请求 | 响应 |
| --- | --- | --- | --- |
| `GET` | `/api/settings` | - | `AppConfig` |
| `PUT` | `/api/settings` | `AppConfig` | `AppConfig` |
| `GET` | `/api/settings/jsonrpc-token` | - | `JsonRpcTokenStatus` |
| `PUT` | `/api/settings/jsonrpc-token` | `UpdateJsonRpcTokenRequest` | `JsonRpcTokenStatus` |
| `GET` | `/api/settings/lan-jsonrpc` | - | `LanJsonRpcStatus` |
| `PUT` | `/api/settings/lan-jsonrpc` | `UpdateLanJsonRpcRequest` | `LanJsonRpcMutationResponse` |
| `POST` | `/api/settings/lan-jsonrpc/token` | - | `LanJsonRpcMutationResponse` |
| `GET` | `/api/settings/proxy` | - | `DownloadProxyStatus` |
| `PUT` | `/api/settings/proxy` | `UpdateDownloadProxyRequest` | `DownloadProxyMutationResponse` |
| `DELETE` | `/api/settings/proxy` | - | `204 No Content` |

约定：

- `GET /api/settings` 在没有已保存配置时，会从 `/api/storage/accessible-paths` 对应授权目录中选择默认下载目录：优先选择包含 `/data` 或以 `data` 结尾的目录，其次选择第一个授权目录；授权目录为空时才回退到 server 应用数据目录。
- `PUT /api/settings` 的 `defaultDownloadDir` 必须来自已授权目录；授权目录为空时只允许使用 server 应用数据目录。未授权目录返回 `400 Bad Request`，错误码为 `settings_save_failed`。
- `language` 为 Web UI 语言偏好，当前支持 `zh-CN` 和 `en-US`；旧配置或非法值会回退为 `zh-CN`。
- `GET /api/settings` 和 `PUT /api/settings` 不接收、不返回 JSON-RPC Token；旧请求中的 `jsonRpcToken` 字段必须忽略或拒绝，不得回显原文。
- JSON-RPC Token 通过专用受保护接口更新；保存后立即生效且无需重启 Aria2。
- `GET /api/settings/jsonrpc-token` 只返回是否已配置和掩码，不返回 Token 原文。
- JSON-RPC Token 为空时，`/jsonrpc` 的 `aria2.addUri` 会拒绝添加任务；`aria2.getVersion` 仍可用于连通性测试。
- 局域网 JSON-RPC 配置使用独立的 `jsonrpc_lan` 持久化记录。首次启用且尚无 Token 时由服务端生成 32 字节随机 Token；关闭入口保留 Token，重新启用不影响已配置客户端。
- `issuedToken` 只在首次生成或主动轮换时返回一次；普通读取、关闭和使用既有 Token 重新启用时必须为 `null`。任何响应和日志均不得回传或记录旧 Token 原文。
- 下载代理以独立 `app_config` 键 `download_proxy` 保存规范化 URL、单调递增 revision 和更新时间，不属于公开 `AppConfig`。普通 `/api/settings` 不接收或返回该记录。
- 代理 URL trim 后最长 2048 字节，使用结构化 URL 解析；只接受 `http`、`https`、`socks4`、`socks5`，要求合法 host 和 port，拒绝 query、fragment 与控制字符。允许 userinfo，但规范化不得改变凭据大小写或含义。
- 保存相同的规范化 URL 是无变化操作：不增加 revision、不调用 Aria2、不触发任务重连。并发保存按数据库事务串行化 revision。
- 替换配置本身成功后，不启动已停止的 Aria2。已运行且来源为应用配置的启用任务分别进入 `appliedTaskIds`、`deferredTaskIds` 或脱敏的 `failed`；部分即时应用失败不回滚已保存配置，后续在恢复、继续或显式操作时重新对账。
- 仍有 `useProxy=true` 且来源为应用配置的任务时，包含完成和回收站任务，清除配置返回 `409 proxy_in_use`。兼容私密覆盖任务不构成引用。成功清除返回 `204 No Content`。
- 代理设置加载或保存失败使用内部错误码 `proxy_load_failed`、`proxy_save_failed`；任何错误响应、日志和调试记录都不得包含代理原文或凭据。

`AppConfig`：

```json
{
  "defaultDownloadDir": "/vol1/downloads",
  "maxConcurrentDownloads": 5,
  "downloadLimit": 0,
  "uploadLimit": 0,
  "language": "zh-CN"
}
```

`JsonRpcTokenStatus`：

```json
{
  "configured": true,
  "maskedToken": "••••••••a1b2"
}
```

`UpdateJsonRpcTokenRequest`：

```json
{
  "token": "new-json-rpc-token"
}
```

`LanJsonRpcStatus`：

```json
{
  "enabled": true,
  "configured": true,
  "maskedToken": "••••••••a1b2",
  "port": 17082
}
```

`UpdateLanJsonRpcRequest` 与 `LanJsonRpcMutationResponse`：

```json
{
  "enabled": true
}
```

```json
{
  "status": {
    "enabled": true,
    "configured": true,
    "maskedToken": "••••••••a1b2",
    "port": 17082
  },
  "issuedToken": "one-time-raw-token-or-null"
}
```

`DownloadProxyStatus`：

```json
{
  "configured": true,
  "maskedProxyUrl": "http://***:***@proxy.example.com:7890",
  "revision": 4
}
```

未配置时 `configured=false`、`maskedProxyUrl=null`、`revision=0`。用户名和密码统一掩码，响应不得包含 query、fragment 或可还原凭据的信息。

`UpdateDownloadProxyRequest`：

```json
{
  "proxyUrl": "http://user:password@proxy.example.com:7890"
}
```

`DownloadProxyMutationResponse`：

```json
{
  "status": {
    "configured": true,
    "maskedProxyUrl": "http://***:***@proxy.example.com:7890",
    "revision": 4
  },
  "appliedTaskIds": [1],
  "deferredTaskIds": [2],
  "failed": [
    {
      "taskId": 3,
      "code": "runtime_transition",
      "message": "Aria2 正在切换运行状态，请稍后重试"
    }
  ]
}
```

非法 URL 返回 `400 proxy_invalid_url`；错误消息只描述校验规则，不回显输入。`failed` 只包含任务 ID、稳定错误码和脱敏消息。

### 4.6 调试日志

| 方法 | 路径 | 响应 |
| --- | --- | --- |
| `GET` | `/api/debug-logs` | `DebugLogEntry[]` |
| `DELETE` | `/api/debug-logs` | `204 No Content` |

`DebugLogEntry` 保留原有字段，并包含用于高级筛选和重复折叠的字段：

```json
{
  "id": 1,
  "timestampMs": 1783670013000,
  "lastTimestampMs": 1783670013000,
  "level": "info",
  "category": "task",
  "module": "tasks.create",
  "message": "下载任务已写入内存列表和 SQLite，ID 1，GID abc",
  "repeatCount": 1
}
```

约定：

- `category` 可为 `app`、`task`、`aria2`、`settings`、`storage`、`api`、`runtime`。
- 应用内调试日志与 `app/data/logs/server.log` 默认只记录关键生命周期、用户操作、状态转换、警告和错误；应用信息、通信检查、设置读取、Aria2 状态、`aria2.getVersion` 和 `aria2.getGlobalOption` 等常规只读成功请求不写文件日志。`server.log` 单文件上限为 10 MiB，保留当前文件和最多 3 个历史文件；fnOS 生命周期脚本和进程标准输出进入同目录的 `lifecycle.log`，默认单文件上限为 1 MiB，也保留最多 3 个历史文件。
- 文件日志和内存调试日志共用敏感字段脱敏规则：URL 的 query/fragment、Token、密码、Session、CSRF、Cookie、Authorization 和 RPC secret 不写入日志。排障时可用响应头 `X-Request-ID` 将管理 API、SSE 和 JSON-RPC 请求与日志关联。
- 下载代理 URL、userinfo、兼容私密覆盖、应用配置 revision 与 Aria2 `all-proxy` 值不进入普通操作日志、调试日志或诊断响应；任务相关记录只允许使用 `useProxy` 布尔值。
- 连续相同级别、模块和消息会折叠为一条，`repeatCount` 记录次数，`lastTimestampMs` 记录最后发生时间。

### 4.7 存储目录

| 方法 | 路径 | 响应 |
| --- | --- | --- |
| `GET` | `/api/storage/accessible-paths` | `AccessiblePathsResponse` |

`AccessiblePathsResponse`：

```json
{
  "paths": ["/vol1/downloads", "/vol1/media"]
}
```

约定：

- `paths` 来自 fnOS 应用设置中授予 Motrix 的文件夹访问权限。
- 返回值会去掉空路径和重复路径。
- 前端新建任务时必须从 `paths` 中选择保存目录；如果列表为空，应提示用户先在 fnOS 应用设置中添加读写文件夹授权。

## 5. SSE 事件流

| 方法 | 路径 | 说明 |
| --- | --- | --- |
| `GET` | `/api/events` | 订阅运行时事件流 |

约定：

- 格式使用标准 SSE。
- 连接建立后立即推送一次 `tasks.snapshot`。
- 可见任务列表发生变化时推送 `tasks.snapshot`。
- 服务进入退出流程时推送 `runtime.exiting`。
- 当前事件模型使用整包快照，不使用增量 diff。
- Session 失效时连接终止；重新订阅返回 `401 Unauthorized`，不会发送任务快照。

`tasks.snapshot`：

```json
{
  "revision": 1,
  "tasks": []
}
```

`runtime.exiting`：

```json
{
  "reason": "收到停止信号",
  "timestamp": 1760000000000
}
```

## 6. JSON-RPC 兼容入口

`/jsonrpc` 是为解析站、浏览器扩展或外部工具提供的 Aria2 JSON-RPC 兼容入口，不属于 Web UI 的主通信路径。公网反代入口注册在 `127.0.0.1:17081`，局域网入口注册在 `0.0.0.0:17082`；Web UI 仍通过管理监听器的 `/api/*` 和 `/api/events` 工作。

不兼容变更：JSON-RPC 客户端必须迁移到指向回环专用监听器的反向代理；Web 管理首次使用必须先设置独立管理密码。

| 方法 | 路径 | 说明 |
| --- | --- | --- |
| `POST` | `/jsonrpc` | 接收 JSON-RPC 2.0 请求或批量请求 |
| `GET` | `/jsonrpc` | WebSocket JSON-RPC；支持 `jsonrpc` 子协议 |
| `OPTIONS` | `/jsonrpc` | 预检请求，返回跨域所需响应头 |

支持方法：

| JSON-RPC 方法 | 鉴权 | 说明 |
| --- | --- | --- |
| `aria2.addUri` | 需要 `jsonRpcToken` | 添加 HTTP/HTTPS 或磁力链接下载任务，成功返回 Aria2 GID |
| `aria2.getGlobalOption` | 需要 `jsonRpcToken` | 从内存返回安全兼容子集，目前只包含已授权默认下载目录 `dir`；没有可用授权目录时返回空字符串，不启动或探测 Aria2，不返回 RPC secret、代理凭据等敏感配置 |
| `aria2.getVersion` | 不需要 | 运行时返回版本与空 `enabledFeatures` 并更新进程内版本缓存；已停止时不启动 Aria2、不访问磁盘，返回最后一次读取到的版本，尚无缓存时返回 `unknown`；正在停止时返回 `-32004` 和 `Aria2 正在停止，请稍后重试` |
| `system.multicall` | 子调用按方法校验 | 批量执行；其中每个需要鉴权的子调用都必须在自身参数中携带有效 token |

鉴权约定：

- `jsonRpcToken` 通过 `/api/settings/jsonrpc-token` 专用接口更新，不是 Web 管理密码或 Aria2 RPC Secret，也不会暴露后端内部 Aria2 secret。
- `lanJsonRpcToken` 由 `/api/settings/lan-jsonrpc` 首次启用时生成，或通过 `/api/settings/lan-jsonrpc/token` 轮换；它只在 `17082` 有效，公网 Token 只在 `17081` 有效。
- 两类 Token 在服务启动时加载到内存，设置成功后同步更新；所有鉴权方法都使用常量时间比较，请求过程中不得读取 SQLite。
- `aria2.addUri` 的第一个参数必须是 `"token:<jsonRpcToken>"`；token 缺失、错误或未配置会返回 JSON-RPC error。
- `aria2.addUri` 选项中的旧 `all-proxy` 继续生效，其最终值只进入任务私密覆盖记录；任务响应、SSE 和日志仅暴露 `useProxy=true`。没有 `all-proxy` 的外部创建按 `useProxy=false` 保存。代理值校验失败返回 JSON-RPC `-32602`，不得创建 GID 或任务记录。
- `aria2.getGlobalOption` 同样要求第一个参数为 `"token:<jsonRpcToken>"`；目录与 Token 在服务启动时加载到内存，并在管理设置保存成功后同步更新，因此重复查询不会读取 SQLite、授权目录文件或唤醒 Aria2。应用数据根目录不会作为外部下载目录返回；没有可用授权目录时 `dir` 为 `""`，发送端应按未指定目录处理。
- fnOS 在服务运行期间调整授权目录时，外部发送端可能短暂携带上一次查询到的旧默认目录；`aria2.addUri` 仅在该值确实等于服务端曾返回的缓存默认目录时改用当前授权默认目录并刷新缓存。其他未授权目录仍返回 `-32602`。
- `aria2.getVersion` 保持匿名可用；HTTP、WebSocket 和 `system.multicall` 在已停止时使用相同的只读兼容结果，在正在停止时使用相同的 `-32004` 错误。
- `system.multicall` 外层 token 会被忽略；每个 `aria2.addUri` 或 `aria2.getGlobalOption` 子调用仍需在自身 `params` 中携带 token。

`aria2.addUri` 示例：

```json
{
  "jsonrpc": "2.0",
  "id": "add-1",
  "method": "aria2.addUri",
  "params": [
    "token:your-json-rpc-token",
    ["https://example.com/file.zip"],
    {
      "dir": "/vol1/downloads",
      "out": "file.zip",
      "split": "8",
      "max-connection-per-server": "8"
    }
  ]
}
```

约定：

- `dir` 必须来自 `/api/storage/accessible-paths` 返回的授权目录；未传 `dir` 时使用后端默认下载目录，并同样要求该目录已授权。
- 为兼容会删除 Unix 路径首个 `/` 的第三方发送页，JSON-RPC 仅在请求值不含空组件、`.`、`..` 或反斜杠，且补回一个 `/` 后能唯一、精确匹配授权目录时接受该值；任务最终仍使用授权列表中的原始绝对路径，不允许借此访问授权目录的任意子目录。
- `out` 会映射为 Motrix 任务文件名。
- 当 URL 为 `magnet:?` 时，`dir` 表示授权父目录；后端会创建任务专属子目录，启用 metadata 暂停和 `bt-save-metadata`，待解析完成后仍通过 Web UI 的文件确认流程开始真实下载。
- 远程入口只支持 HTTP / HTTPS URL 和 `magnet:?`，不支持上传种子文件；种子文件使用 Web UI 或 `/api/tasks/torrent`。
- 只透传常用下载加速与请求参数；未知选项、空值、对象值会被忽略。
- 不支持的方法返回 `-32601 Method not found`；参数错误返回 `-32602 Invalid params`；服务侧错误返回 `-32000`；token 错误返回 `-32001`，token 未配置返回 `-32002`；Aria2 正在停止时返回 `-32004`。
- 不要在公开网页、前端仓库或日志中记录 `jsonRpcToken`；公网反向代理只能指向回环 RPC 专用监听器的 `/jsonrpc`，根路径、`/api/*`、SSE 和静态资源在该监听器上必须保持 404。
- 局域网入口关闭时所有请求返回 404；开启时只接受 RFC1918 IPv4 真实对端。它不支持 IPv6、链路本地、回环或通过代理 Header 扩展来源范围。
