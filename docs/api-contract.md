# 前后端 HTTP / SSE / JSON-RPC API 契约

> 本文档定义 Rust server、Vue Web UI 与外部 JSON-RPC 兼容调用方之间的接口边界。总体架构见 `docs/architecture.md`；阶段状态见 `docs/development-plan.md`；UI 产品需求见 `docs/design/ui-product-requirements.md`；FPK 构建与产物见 `docs/fpk-packaging.md`。

## 1. 运行时约定

| 环境变量 | 作用 | 默认值 |
| --- | --- | --- |
| `MOTRIX_FNOS_APP_DATA_DIR` | server 数据目录 | 用户本地数据目录下的 `motrix-fnos` |
| `MOTRIX_FNOS_HTTP_ADDR` | HTTP 监听地址 | `127.0.0.1:17080` |
| `MOTRIX_FNOS_ARIA2_PATH` | Aria2 可执行文件路径 | 打包路径优先，仓库调试路径兜底 |
| `MOTRIX_FNOS_ACCESSIBLE_PATHS_FILE` | fnOS 已授权目录快照文件 | `MOTRIX_FNOS_APP_DATA_DIR/accessible-paths.json` |

FPK 脚本从 fnOS 注入的 `TRIM_DATA_ACCESSIBLE_PATHS` 读取已授权目录，并写入 `MOTRIX_FNOS_ACCESSIBLE_PATHS_FILE`。后端以该文件为主，文件不存在时才回退读取当前进程环境变量。

## 2. 前端消费约定

- FPK Web UI 从 manifest `service_port` 对应的应用端口访问后端，API 与 SSE 使用同源 `/api/*` 和 `/api/events`。
- FPK 桌面入口与 Rust server 使用同一端口；不得再同时声明统一网关字段并把该端口限制成仅 JSON-RPC。
- `/jsonrpc` 与 Web UI 共用服务端口，写操作继续要求 JSON-RPC token。
- 开发态由 Vite proxy 转发 `/api` 与 `/api/events` 到本地 server。
- JSON 接口使用浏览器原生 `fetch`。
- SSE 使用浏览器原生 `EventSource`。
- 错误提示优先展示响应体中的 `message`。
- 下载保存目录只允许从后端返回的 fnOS 已授权目录中选择；Web UI 不提供任意本地路径选择器。

## 3. 错误响应

统一错误响应：

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
| `409 Conflict` | 当前运行状态不允许执行该操作 |
| `500 Internal Server Error` | 未预期内部错误 |

`204 No Content` 响应不带 JSON body。

## 4. HTTP API

下表使用同源 `/api` 路径，FPK 与开发态保持一致。

### 4.1 应用信息

| 方法 | 路径 | 响应 |
| --- | --- | --- |
| `GET` | `/api/app/info` | `AppInfo` |
| `GET` | `/api/app/ping` | `BackendPing` |
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
      "name": "motrix.fnos_1.3.4_x86.fpk",
      "downloadUrl": "https://github.com/RockerHX/motrix-fnos/releases/download/v1.3.4/motrix.fnos_1.3.4_x86.fpk"
    },
    {
      "architecture": "arm",
      "name": "motrix.fnos_1.3.4_arm.fpk",
      "downloadUrl": "https://github.com/RockerHX/motrix-fnos/releases/download/v1.3.4/motrix.fnos_1.3.4_arm.fpk"
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

### 4.2 Aria2

| 方法 | 路径 | 说明 | 响应 |
| --- | --- | --- | --- |
| `GET` | `/api/aria2/config` | 读取 Aria2 配置状态 | `Aria2ConfigStatus` |
| `GET` | `/api/aria2/process` | 读取受管进程状态 | `Aria2ProcessStatus` |
| `GET` | `/api/aria2/rpc` | 读取 RPC 连通状态 | `Aria2RpcStatus` |
| `POST` | `/api/aria2/start` | 启动受管 Aria2 | `Aria2ProcessStatus` |
| `POST` | `/api/aria2/stop` | 停止受管 Aria2 | `Aria2ProcessStatus` |

### 4.3 任务

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
| `POST` | `/api/tasks/:id/redownload` | - | `DownloadTask` |
| `DELETE` | `/api/tasks/:id?deleteFiles=true|false` | - | `DownloadTask` |
| `DELETE` | `/api/tasks/:id/permanent` | - | `204 No Content` |

约定：

- `GET /api/tasks` 只返回未删除任务。
- `GET /api/tasks?status=removed` 只返回已删除任务记录，用于回收站页面。
- `status` 当前只支持 `removed`；其他值返回 `400 Bad Request`。
- `DELETE /api/tasks/:id/permanent` 只允许永久删除已删除任务记录；该操作只清理 Motrix 数据库记录，不删除用户下载文件。
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
    "proxy": ""
  }
}
```

约定：

- `sourceType` 可选值为 `url` / `magnet`，省略时兼容旧请求并按 `url` 处理。
- `startMode` 可选值为 `now` / `paused`，省略时按 `now` 处理。
- `category` 是 Motrix 任务标签，默认 `默认`；它不改变保存目录，也不影响侧栏状态分类。
- `advancedOptions.connections` 映射 Aria2 `split` 与 `max-connection-per-server`；`advancedOptions.downloadLimitKb` 映射单任务下载限速；`advancedOptions.proxy` 映射 `all-proxy`。
- `saveDir` 必须来自 `/api/storage/accessible-paths` 返回的 `paths`；为空或未授权路径会返回 `400 Bad Request`。
- 当 `sourceType=magnet` 时，请求中的 `saveDir` 表示授权父目录；成功创建后返回的 `DownloadTask.saveDir` 是后端创建的任务专属子目录。
- `aria2Options` 为兼容字段；Web UI 不直接使用该字段，外部调用或 `/jsonrpc` 兼容入口可传入受支持的 Aria2 参数。
- 后端只透传白名单内的 Aria2 选项，并会覆盖 `dir` / `out`，确保保存目录和文件名仍由 Motrix 校验。

`DownloadTask`：

```json
{
  "id": 1,
  "url": "https://example.com/file.zip",
  "fileName": "file.zip",
  "saveDir": "/vol1/downloads",
  "category": "默认",
  "gid": "abc123",
  "status": "pending",
  "totalLength": 1024,
  "completedLength": 0,
  "downloadSpeed": 0,
  "errorCode": null,
  "errorMessage": null,
  "filePath": "/vol1/downloads/file.zip",
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
    "proxy": ""
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

### 4.4 设置

| 方法 | 路径 | 请求 | 响应 |
| --- | --- | --- | --- |
| `GET` | `/api/settings` | - | `AppConfig` |
| `PUT` | `/api/settings` | `AppConfig` | `AppConfig` |

约定：

- `GET /api/settings` 在没有已保存配置时，会从 `/api/storage/accessible-paths` 对应授权目录中选择默认下载目录：优先选择包含 `/data` 或以 `data` 结尾的目录，其次选择第一个授权目录；授权目录为空时才回退到 server 应用数据目录。
- `PUT /api/settings` 的 `defaultDownloadDir` 必须来自已授权目录；授权目录为空时只允许使用 server 应用数据目录。未授权目录返回 `400 Bad Request`，错误码为 `settings_save_failed`。
- `language` 为 Web UI 语言偏好，当前支持 `zh-CN` 和 `en-US`；旧配置或非法值会回退为 `zh-CN`。
- `jsonRpcToken` 用于公网 `/jsonrpc` 添加任务鉴权；它不是 Aria2 RPC Secret，不会传给 Aria2，保存后立即生效且无需重启 Aria2。
- `jsonRpcToken` 为空时，`/jsonrpc` 的 `aria2.addUri` 会拒绝添加任务；`aria2.getVersion` 仍可用于连通性测试。

`AppConfig`：

```json
{
  "defaultDownloadDir": "/vol1/downloads",
  "maxConcurrentDownloads": 5,
  "downloadLimit": 0,
  "uploadLimit": 0,
  "language": "zh-CN",
  "jsonRpcToken": ""
}
```

### 4.5 调试日志

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
- 应用内调试日志面向用户排障，默认只保留关键生命周期、用户操作、警告和错误；高频健康检查等详细运行轨迹进入 `app/data/logs/server.log`。
- 连续相同级别、模块和消息会折叠为一条，`repeatCount` 记录次数，`lastTimestampMs` 记录最后发生时间。

### 4.6 存储目录

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

`tasks.snapshot`：

```json
{
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

`/jsonrpc` 是为解析站、浏览器扩展或外部工具提供的 Aria2 JSON-RPC 兼容入口，不属于 Web UI 的主通信路径。Web UI 仍通过 `/api/*` 和 `/api/events` 工作。

| 方法 | 路径 | 说明 |
| --- | --- | --- |
| `POST` | `/jsonrpc` | 接收 JSON-RPC 2.0 请求或批量请求 |
| `GET` | `/jsonrpc` | WebSocket JSON-RPC；支持 `jsonrpc` 子协议 |
| `OPTIONS` | `/jsonrpc` | 预检请求，返回跨域所需响应头 |

支持方法：

| JSON-RPC 方法 | 鉴权 | 说明 |
| --- | --- | --- |
| `aria2.addUri` | 需要 `jsonRpcToken` | 添加 HTTP/HTTPS 或磁力链接下载任务，成功返回 Aria2 GID |
| `aria2.getVersion` | 不需要 | 连通性测试，返回版本与空 `enabledFeatures` |
| `system.multicall` | 子调用按方法校验 | 批量执行；其中每个 `aria2.addUri` 子调用都必须携带有效 token |

鉴权约定：

- `jsonRpcToken` 通过 `/api/settings` 保存，不是 Aria2 RPC Secret，也不会暴露后端内部 Aria2 secret。
- `aria2.addUri` 的第一个参数必须是 `"token:<jsonRpcToken>"`；token 缺失、错误或未配置会返回 JSON-RPC error。
- `aria2.getVersion` 保持匿名可用，便于外网连通性测试。
- `system.multicall` 外层 token 会被忽略；每个 `aria2.addUri` 子调用仍需在自身 `params` 中携带 token。

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
- `out` 会映射为 Motrix 任务文件名。
- 当 URL 为 `magnet:?` 时，`dir` 表示授权父目录；后端会创建任务专属子目录，启用 metadata 暂停和 `bt-save-metadata`，待解析完成后仍通过 Web UI 的文件确认流程开始真实下载。
- 远程入口只支持 HTTP / HTTPS URL 和 `magnet:?`，不支持上传种子文件；种子文件使用 Web UI 或 `/api/tasks/torrent`。
- 只透传常用下载加速与请求参数；未知选项、空值、对象值会被忽略。
- 不支持的方法返回 `-32601 Method not found`；参数错误返回 `-32602 Invalid params`；服务侧错误返回 `-32000`；token 错误返回 `-32001`，token 未配置返回 `-32002`。
- 不要在公开网页、前端仓库或日志中记录 `jsonRpcToken`；对公网开放独立端口前，必须确认 fnOS 网络、防火墙和反向代理访问控制。
