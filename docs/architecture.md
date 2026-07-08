# 飞牛版 Motrix 架构文档

> 本文档只约束长期架构、职责边界、目录组织和运行模型。阶段任务、状态和优先级见 `docs/development-plan.md`；接口细节见 `docs/api-contract.md`；打包命令见 `docs/fpk-packaging.md`；历史设计归档参考见 `docs/design/archive/ui-stitch-prompts.md`。

## 1. 架构边界

本项目交付 **fnOS FPK 下载管理应用**。

固定边界：

- 交付形态：`.fpk`。
- 运行模型：fnOS 服务启动 Rust server，server 托管 Web UI 并管理 Aria2 Next sidecar。
- 前后端通信：HTTP API + SSE。
- 长期状态：SQLite 与 Aria2 session 持久化到 FPK 应用数据目录。
- 维护主线：`server/`、`src/`、`packaging/fnos/`。

## 2. 技术栈

| 层级 | 选型 |
| --- | --- |
| FPK | `packaging/fnos/`、`cmd/start`、`cmd/stop`、`cmd/status` |
| 后端 | Rust、Tokio、Axum、Serde、SQLx、SQLite、tracing |
| 下载引擎 | Aria2 Next sidecar，通过 JSON-RPC 控制 |
| 前端 | Vue 3、TypeScript、Vite、Naive UI、Pinia |
| 通信 | HTTP API、SSE |

## 3. 运行拓扑

```text
fnOS FPK
  ├─ manifest / config / cmd / wizard / icons
  ├─ Rust server
  │   ├─ Axum HTTP API
  │   ├─ SSE 事件流
  │   ├─ Aria2 Next 进程管理
  │   ├─ SQLite 持久化
  │   └─ 调试日志与运行状态
  ├─ Web UI
  │   └─ Vue 3 + Naive UI + Pinia
  └─ Aria2 Next Linux sidecar
```

### 3.1 FPK 目录约定

```text
packaging/fnos/
  manifest
  config/
  cmd/
  wizard/
  app/
    bin/          # motrix-fnos-server 与 aria2-next
    ui/dist/      # Web UI 静态资源
    data/         # 运行时数据目录
  dist/           # .fpk 输出
```

约定：

- `cmd/start` 启动 Rust server，并注入数据目录、监听地址和 Aria2 路径。
- `cmd/stop` 触发 server 统一退出流程。
- `cmd/status` 只判断服务进程状态。
- `app/data/` 保存 SQLite、Aria2 session、日志、PID 等运行态文件；打包前不得携带本地残留。
- Web UI 由 Rust server 托管，通过相对路径访问 `/api/*` 和 `/api/events`。

### 3.2 架构与产物匹配

| 设备架构 | Rust target | FPK 输出 |
| --- | --- | --- |
| `x86_64` | `x86_64-unknown-linux-gnu` | `motrix.fnos_<version>_x86.fpk` |
| `aarch64` / `arm64` | `aarch64-unknown-linux-gnu` | `motrix.fnos_<version>_arm.fpk` |

安装时必须选择与设备 CPU 架构匹配的 FPK。

## 4. 分层职责

| 层级 | 职责 | 不承担 |
| --- | --- | --- |
| FPK 打包层 | manifest、权限、图标、Web 入口、服务脚本、产物组装 | 下载业务、页面交互、业务数据持久化 |
| Rust server | 任务生命周期、配置校验、路径安全、Aria2 进程与 RPC、SQLite、日志、HTTP/SSE | 页面布局、组件交互、前端临时状态 |
| Aria2 Next | HTTP/HTTPS/BT/磁力下载、断点续传、限速、状态上报 | UI、配置/历史存储、fnOS 生命周期 |
| Vue Web UI | 页面展示、用户交互、轻量输入反馈、调用 service/store | 下载执行、文件系统权限判断、数据库读写 |
| Pinia | 前端任务状态、筛选、选中项、事件订阅状态、配置缓存、UI 偏好 | SQLite 持久化、Aria2 RPC、复杂后端判断 |
| SQLite | 任务记录、配置、历史、错误记录、需长期保存的 UI 偏好 | 实时下载执行、页面临时状态 |

## 5. 前端约束

目标目录：

```text
src/
  app/providers/
  layouts/
  views/
  features/
    tasks/
    diagnostics/
    settings/
  services/
  types/
```

约束：

- `views/` 只放页面入口，负责组合布局和功能模块。
- `layouts/` 放通用页面结构。
- `features/` 按业务领域拆分组件、store、service、composable 和类型。
- `services/` 放 HTTP client 和运行时事件订阅封装。
- `MainWindow.vue` 只承担页面编排，不直接实现任务表、复杂弹窗、Toast 队列、任务轮询或后端接口调用。
- 桌面 Web、手机浏览器和飞牛 App WebView 共用同一套 Vue Web UI 源码、Pinia store、service、HTTP API 和 SSE 数据流；不得为手机端另建独立前端工程、独立业务状态或独立后端接口。
- 响应式适配优先在 `layouts/` 和 `features/*` 展示组件内完成：布局外壳可按桌面/移动拆分组件，信息结构差异明显的功能组件可拆桌面/移动展示组件，但业务操作必须复用同一 store/service。
- UI 优先使用 Naive UI；自定义 CSS 仅用于整体主题、侧栏、shell、颜色、间距和圆角。

## 6. 后端约束

目标目录：

```text
server/
  src/
    main.rs
    state.rs
    api/
    runtime/
    services/
    tasks/
    aria2/
    config/
    database/
    debug_logs/
```

约束：

- `api/` 只负责 HTTP handler 和请求/响应转换。
- `services/` 负责编排业务流程。
- `tasks/`、`aria2/`、`config/`、`database/`、`debug_logs/` 保持领域边界。
- 新增后端能力按 `api -> service -> domain -> persistence` 分层。

## 7. 数据流与事件流

标准数据流：

```text
Vue Component
  -> Pinia Store
  -> Feature Service
  -> HTTP client
  -> Axum Route
  -> Rust Service / Repository
  -> Aria2 JSON-RPC / SQLite
```

任务创建约束：

- 单链接、磁力、批量 URL、种子文件等“新建任务”入口可以在 API / service 编排层分流，但底层创建流程必须尽量复用统一的任务创建链路，避免再维护第二套校验、Aria2 option 映射、内存态写入和 SQLite 持久化逻辑。
- 批量 URL 本质上是批量创建多个独立单任务：后端逐条校验、逐条创建，允许部分成功 / 部分失败；单条失败不回滚已成功创建的任务。
- 磁力链接任务遵循 Aria2 原生 metadata 流程：先添加 magnet metadata 任务，metadata 完成后通过 `followedBy` 切换到真实 BT GID；真实任务保持暂停并由前端展示文件列表，用户确认后后端再设置 `select-file` 并恢复下载。
- 种子文件任务以用户选择的授权目录作为父目录，后端创建任务专属子目录承载 Aria2 原生 hash 命名 `.torrent` 元数据、`.aria2` 控制文件和下载产物；勾选删除文件时只允许删除该任务专属目录，不得删除授权目录根。为保持 Aria2 session 恢复语义，不额外改名或删除 Aria2 自动保存的种子元数据。

标准事件流：

```text
Rust Runtime Event
  -> SSE
  -> frontend runtime event service
  -> Pinia Store
  -> Components
```

禁止：

- Vue 组件散落直接调用后端接口。
- 前端直接拼装复杂 Aria2 RPC 请求。
- Rust handler 内堆积业务逻辑。
- 混用 UI 临时状态和后端持久状态。

## 8. 生命周期与安全边界

- 应用启动、停止和状态查询以 fnOS `start` / `stop` / `status` 为准。
- 后端启动时准备数据目录、初始化 SQLite、启动或连接 Aria2。
- 后端停止时保存任务状态、保存 Aria2 session、停止当前服务管理的 Aria2 实例。
- 前端页面关闭、刷新或重新进入不等于应用退出。
- SQLite、Aria2 session、Aria2 log 和运行态文件必须放在 FPK 应用数据目录。
- 下载目录不能写死桌面用户目录，必须使用 fnOS 可访问目录或应用数据目录下的默认下载区。
- Aria2 RPC secret 只能由服务端生成和持有，不暴露给前端。
- 日志必须隐藏私密 URL query 和敏感配置。

## 9. fnOS 平台查证规则

涉及 fnOS / FPK / 应用中心 / manifest / `config/resource` / `config/privilege` / `cmd/*` 生命周期 / `TRIM_*` 环境变量 / 文件夹授权 / 端口入口 / 安装、升级、卸载行为时，必须先查证资料或实机验证，不能只凭通用 Linux、NAS 或既有记忆下结论。

资料优先级：

1. 飞牛官方资料，优先查看飞牛应用开放平台开发文档：https://developer.fnnas.com/docs/guide/
2. 可验证第三方 FPK 仓库、社区指南、论坛排障记录。
3. 本仓库实证：解包、实机日志、`fnpack` 行为、安装目录和环境变量。
4. 推断：仅在前三者不足时使用，并必须说明验证步骤。

修改涉及 fnOS 平台行为的实现或文档时，应同步记录查证来源或验证方式。

## 10. 开发约束

- 新增前端交互进入 `features/*`，不得重新向入口页面堆叠。
- 新增通信能力默认走 HTTP API / SSE。
- 新增长期状态必须考虑 SQLite 持久化路径和迁移策略。
- 若本文档与实际演进不匹配，先更新本文档，再继续实现。
