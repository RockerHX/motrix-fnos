# 前后端 HTTP / SSE / JSON-RPC API 契约

> 本文档定义 Rust server、Vue Web UI 与外部 JSON-RPC 兼容调用方之间的接口边界。总体架构见 `docs/architecture.md`；FPK 构建与产物见 `docs/fpk-packaging.md`。

## 1. 运行时约定

| 环境变量 | 作用 | 默认值 |
| --- | --- | --- |
| `MOTRIX_FNOS_APP_DATA_DIR` | server 数据目录 | 用户本地数据目录下的 `motrix-fnos` |
| `MOTRIX_FNOS_HTTP_ADDR` | HTTP 监听地址 | `127.0.0.1:17080` |
| `MOTRIX_FNOS_ARIA2_PATH` | Aria2 可执行文件路径 | 打包路径优先，仓库调试路径兜底 |
| `MOTRIX_FNOS_ACCESSIBLE_PATHS_FILE` | fnOS 已授权目录快照文件 | `MOTRIX_FNOS_APP_DATA_DIR/accessible-paths.json` |

FPK 脚本从 fnOS 注入的 `TRIM_DATA_ACCESSIBLE_PATHS` 读取已授权目录，并写入 `MOTRIX_FNOS_ACCESSIBLE_PATHS_FILE`。后端以该文件为主，文件不存在时才回退读取当前进程环境变量。

## 2. 前端消费约定

- Web UI 通过相对路径访问后端：`/api/*` 和 `/api/events`。
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

所有 HTTP 路由均以 `/api` 为前缀，当前不设置显式版本号。

### 4.1 应用信息

| 方法 | 路径 | 响应 |
| --- | --- | --- |
| `GET` | `/api/app/info` | `AppInfo` |
| `GET` | `/api/app/ping` | `BackendPing` |

示例：

```json
{
  "name": "Motrix",
  "version": "1.2.0",
  "backendStatus": "ready"
}
```

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

`CreateDownloadTaskRequest`：

```json
{
  "url": "https://example.com/file.zip",
  "fileName": "file.zip",
  "saveDir": "/vol1/downloads",
  "aria2Options": {
    "split": "8",
    "max-connection-per-server": "8"
  }
}
```

约定：

- `saveDir` 必须来自 `/api/storage/accessible-paths` 返回的 `paths`；为空或未授权路径会返回 `400 Bad Request`。
- `aria2Options` 为可选字段；当前 Web UI 创建弹窗不发送该字段，外部调用或 `/jsonrpc` 兼容入口可传入受支持的 Aria2 参数。
- 后端只透传白名单内的 Aria2 选项，并会覆盖 `dir` / `out`，确保保存目录和文件名仍由 Motrix 校验。

### 4.4 设置

| 方法 | 路径 | 请求 | 响应 |
| --- | --- | --- | --- |
| `GET` | `/api/settings` | - | `AppConfig` |
| `PUT` | `/api/settings` | `AppConfig` | `AppConfig` |
| `GET` | `/api/ui-preferences` | - | `UiPreferences` |
| `PUT` | `/api/ui-preferences` | `UiPreferences` | `UiPreferences` |

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
  "autoStartEnabled": false,
  "notificationsEnabled": false,
  "language": "zh-CN",
  "jsonRpcToken": ""
}
```

### 4.5 调试日志

| 方法 | 路径 | 响应 |
| --- | --- | --- |
| `GET` | `/api/debug-logs` | `DebugLogEntry[]` |
| `DELETE` | `/api/debug-logs` | `204 No Content` |

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
| `aria2.addUri` | 需要 `jsonRpcToken` | 添加 HTTP/HTTPS 下载任务，成功返回 Aria2 GID |
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
- 只透传常用下载加速与请求参数；未知选项、空值、对象值会被忽略。
- 不支持的方法返回 `-32601 Method not found`；参数错误返回 `-32602 Invalid params`；服务侧错误返回 `-32000`；token 错误返回 `-32001`，token 未配置返回 `-32002`。
