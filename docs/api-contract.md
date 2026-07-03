# 前后端 HTTP / SSE API 契约

> 本文档定义 Rust server 与 Vue Web UI 之间的接口边界。总体架构见 `docs/architecture.md`；FPK 构建与产物见 `docs/fpk-packaging.md`；实机验证项见 `docs/fnos-manual-test-checklist.md`。

## 1. 运行时约定

| 环境变量 | 作用 | 默认值 |
| --- | --- | --- |
| `MOTRIX_FNOS_APP_DATA_DIR` | server 数据目录 | 用户本地数据目录下的 `motrix-fnos` |
| `MOTRIX_FNOS_HTTP_ADDR` | HTTP 监听地址 | `127.0.0.1:17080` |
| `MOTRIX_FNOS_ARIA2_PATH` | Aria2 可执行文件路径 | 打包路径优先，仓库调试路径兜底 |

## 2. 前端消费约定

- Web UI 通过相对路径访问后端：`/api/*` 和 `/api/events`。
- 开发态由 Vite proxy 转发 `/api` 与 `/api/events` 到本地 server。
- JSON 接口使用浏览器原生 `fetch`。
- SSE 使用浏览器原生 `EventSource`。
- 错误提示优先展示响应体中的 `message`。
- 目录选择、系统通知、开机自启等系统集成能力在 Web UI 中只保留安全降级行为。

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
  "name": "Motrix FNOS",
  "version": "0.1.0",
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
| `POST` | `/api/tasks` | `CreateDownloadTaskRequest` | `DownloadTask` |
| `POST` | `/api/tasks/:id/pause` | - | `DownloadTask` |
| `POST` | `/api/tasks/:id/resume` | - | `DownloadTask` |
| `POST` | `/api/tasks/:id/redownload` | - | `DownloadTask` |
| `DELETE` | `/api/tasks/:id?deleteFiles=true|false` | - | `DownloadTask` |

`CreateDownloadTaskRequest`：

```json
{
  "url": "https://example.com/file.zip",
  "fileName": "file.zip",
  "saveDir": "/downloads"
}
```

### 4.4 设置

| 方法 | 路径 | 请求 | 响应 |
| --- | --- | --- | --- |
| `GET` | `/api/settings` | - | `AppConfig` |
| `PUT` | `/api/settings` | `AppConfig` | `AppConfig` |
| `GET` | `/api/ui-preferences` | - | `UiPreferences` |
| `PUT` | `/api/ui-preferences` | `UiPreferences` | `UiPreferences` |

### 4.5 调试日志

| 方法 | 路径 | 响应 |
| --- | --- | --- |
| `GET` | `/api/debug-logs` | `DebugLogEntry[]` |
| `DELETE` | `/api/debug-logs` | `204 No Content` |

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
