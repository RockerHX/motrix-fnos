# 前后端 HTTP / SSE API 契约

## 目的

定义 Rust server 与 Vue Web UI 之间的 HTTP API、SSE 事件流和错误响应约定。

## 当前状态

阶段 2 已启动。当前文档定义 server 主线首版 HTTP / SSE 契约，供 `server/` 与后续前端迁移共同遵循。

## 运行时约定

- `MOTRIX_FNOS_APP_DATA_DIR`：server 数据目录；未设置时回退到用户本地数据目录下的 `motrix-fnos`
- `MOTRIX_FNOS_HTTP_ADDR`：监听地址；未设置时回退到 `127.0.0.1:17080`
- `MOTRIX_FNOS_ARIA2_PATH`：显式指定 Aria2 可执行文件；未设置时按“打包路径优先、仓库调试路径兜底”解析

## 错误响应

统一错误响应：

```json
{
  "code": "task_conflict",
  "message": "应用正在退出，不能执行任务操作"
}
```

约定：

- `400 Bad Request`：业务校验失败、请求参数非法
- `409 Conflict`：运行时冲突，例如应用退出中、资源状态不允许当前操作
- `500 Internal Server Error`：未预期内部错误

## HTTP API

所有 HTTP 路由均以 `/api` 为前缀，首版不做显式版本号。

### 应用信息

| 方法 | 路径 | 响应 |
| --- | --- | --- |
| `GET` | `/api/app/info` | `AppInfo` |
| `GET` | `/api/app/ping` | `BackendPing` |

`AppInfo`

```json
{
  "name": "Motrix FNOS",
  "version": "0.1.0",
  "backendStatus": "ready"
}
```

`BackendPing`

```json
{
  "ok": true,
  "message": "Rust 后端通信正常"
}
```

### Aria2

| 方法 | 路径 | 说明 | 响应 |
| --- | --- | --- | --- |
| `GET` | `/api/aria2/config` | 读取当前 Aria2 配置状态 | `Aria2ConfigStatus` |
| `GET` | `/api/aria2/process` | 读取当前受管进程状态 | `Aria2ProcessStatus` |
| `GET` | `/api/aria2/rpc` | 读取 RPC 连通状态 | `Aria2RpcStatus` |
| `POST` | `/api/aria2/start` | 启动受管 Aria2 | `Aria2ProcessStatus` |
| `POST` | `/api/aria2/stop` | 停止受管 Aria2 | `Aria2ProcessStatus` |

### 任务

| 方法 | 路径 | 请求 | 响应 |
| --- | --- | --- | --- |
| `GET` | `/api/tasks` | - | `DownloadTask[]` |
| `POST` | `/api/tasks` | `CreateDownloadTaskRequest` | `DownloadTask` |
| `POST` | `/api/tasks/:id/pause` | - | `DownloadTask` |
| `POST` | `/api/tasks/:id/resume` | - | `DownloadTask` |
| `POST` | `/api/tasks/:id/redownload` | - | `DownloadTask` |
| `DELETE` | `/api/tasks/:id` | `?deleteFiles=true\|false` | `DownloadTask` |

`CreateDownloadTaskRequest`

```json
{
  "url": "https://example.com/file.zip",
  "fileName": "file.zip",
  "saveDir": "/downloads"
}
```

### 设置

| 方法 | 路径 | 请求 | 响应 |
| --- | --- | --- | --- |
| `GET` | `/api/settings` | - | `AppConfig` |
| `PUT` | `/api/settings` | `AppConfig` | `AppConfig` |
| `GET` | `/api/ui-preferences` | - | `UiPreferences` |
| `PUT` | `/api/ui-preferences` | `UiPreferences` | `UiPreferences` |

### 调试日志

| 方法 | 路径 | 响应 |
| --- | --- | --- |
| `GET` | `/api/debug-logs` | `DebugLogEntry[]` |
| `DELETE` | `/api/debug-logs` | `204 No Content` |

## SSE 事件流

| 方法 | 路径 | 说明 |
| --- | --- | --- |
| `GET` | `/api/events` | 订阅运行时事件流 |

约定：

- 事件流格式使用标准 SSE
- 连接建立后立即推送一次 `tasks.snapshot`
- 仅当可见任务列表发生变化时再次推送 `tasks.snapshot`
- 服务进入退出流程时推送一次 `runtime.exiting`

`tasks.snapshot`

```json
{
  "tasks": []
}
```

`runtime.exiting`

```json
{
  "reason": "收到停止信号",
  "timestamp": 1760000000000
}
```

## 兼容策略

- 阶段 2 不替换现有前端 `invoke` / `listen` 调用；HTTP / SSE 契约先服务于 `server/` 主线
- `src-tauri` 继续保留 legacy command 与事件实现，直到阶段 3 前端迁移完成
- 首版 SSE 采用“整包快照”而非增量 diff，避免协议在前后端同时复杂化

## 与其他文档关系

- 总体架构边界见 `docs/architecture.md`。
- FPK 交付与部署要求见 `docs/fpk-packaging.md`。
- 实机验证项见 `docs/fnos-manual-test-checklist.md`。
