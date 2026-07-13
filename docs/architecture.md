# 飞牛版 Motrix 架构文档

> 本文档只约束长期架构、职责边界、目录组织和运行模型。阶段任务、状态和优先级见 `docs/development-plan.md`；接口细节见 `docs/api-contract.md`；打包命令见 `docs/fpk-packaging.md`；当前 UI 产品需求与视觉规则见 `docs/design/ui-product-requirements.md` 和 `docs/design/DESIGN.md`。

## 1. 架构边界

本项目交付 **fnOS FPK 下载管理应用**。

固定边界：

- 交付形态：`.fpk`。
- 运行模型：fnOS 服务启动 Rust server，server 托管 Web UI 并管理 Aria2 Next sidecar。
- 前后端通信：fnOS 统一网关承载 HTTP API + SSE；独立 TCP 端口只承载带 token 的 JSON-RPC 兼容入口。
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

### 3.1 交付与运行约束

- `packaging/fnos/` 只承担 FPK 元数据、权限、生命周期脚本和产物组装；具体目录、命令与产物名称见 `docs/fpk-packaging.md`。
- x86_64 与 ARM64 分别构建 FPK，安装包必须与设备 CPU 架构匹配。
- fnOS 生命周期脚本负责启动、停止和查询 Rust server；停止与状态查询必须联合核对 PID、可执行文件和进程启动时间。
- SQLite、Aria2 session、日志和运行态记录统一保存在应用数据目录，打包产物不得携带本地运行残留。
- Web UI、HTTP API 和 SSE 只通过 fnOS 统一网关访问，并要求已登录管理员身份。
- 独立 TCP 端口只提供带 token 的 `/jsonrpc`，不得暴露 Web UI、HTTP API、SSE 或调试日志。

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
      composables/  # 任务分类、分页、批量操作和顶部操作状态
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
- 主窗口弹窗状态与启动/退出刷新编排放在 `src/views/composables/`；跨页面复用的任务操作仍放在 `features/tasks/composables/`。
- 桌面 Web、手机浏览器和飞牛 App WebView 共用同一套 Vue Web UI 源码、Pinia store、service、HTTP API 和 SSE 数据流；不得为手机端另建独立前端工程、独立业务状态或独立后端接口。
- 响应式适配优先在 `layouts/` 和 `features/*` 展示组件内完成：布局外壳可按桌面/移动拆分组件，信息结构差异明显的功能组件可拆桌面/移动展示组件，但业务操作必须复用同一 store/service。
- UI 优先使用 Naive UI；自定义 CSS 仅用于整体主题、侧栏、shell、颜色、间距和圆角。

## 6. 后端约束

目标目录：

```text
server/
  src/
    main.rs
    app/
    state/
    api/
    runtime/
    tasks/
      service.rs
      service/
        create.rs
        query.rs
        control.rs
        delete.rs
        magnet.rs
    aria2/
    config/
    database/
    debug_logs/
    settings/
    storage/
```

约束：

- `api/` 只负责 HTTP handler 和请求/响应转换。
- 业务编排由各领域的 service 承担，不建立脱离领域的通用业务层。
- `tasks/`、`aria2/`、`settings/`、`storage/`、`database/` 和 `debug_logs/` 保持领域边界。
- `tasks/service.rs` 只保留 `TaskService` 依赖注入、运行态守卫和查询委托；创建、查询、控制、删除与磁链确认流程分别由 `tasks/service/` 子模块承载。
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

- 单链接、批量 URL、磁力和种子入口可以在 API / service 层分流，但必须复用统一的校验、Aria2 option 映射、内存写入和 SQLite 持久化链路。
- 批量 URL 按独立任务逐条创建，允许部分成功；单条失败不回滚已创建任务。
- 磁力任务必须先在应用私有目录解析 metadata，待前端确认文件后，才能在用户授权目录创建真实任务及其专属子目录。解析临时目录必须与任务记录关联，以支持恢复和定向清理。
- 种子任务与确认后的磁力任务都使用任务专属目录保存下载产物和 Aria2 元数据。删除文件时只能删除该任务专属目录或应用私有临时目录，不得删除授权目录根，也不得根据用户输入拼接任意删除路径。
- Aria2 自动保存的种子元数据和 session 恢复语义必须保留；具体请求字段、状态和错误响应见 `docs/api-contract.md`。

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
- PID 运行态记录必须包含进程启动时间；停止服务前必须确认 PID 仍属于当前 server 实例。
- 后端启动时准备数据目录、初始化 SQLite、启动或连接 Aria2。
- 后端停止时保存任务状态、保存 Aria2 session、停止当前服务管理的 Aria2 实例。
- 前端页面关闭、刷新或重新进入不等于应用退出。
- SQLite、Aria2 session、Aria2 log 和运行态文件必须放在 FPK 应用数据目录。
- 下载目录不能写死桌面用户目录，必须使用 fnOS 可访问目录或应用数据目录下的默认下载区。
- Aria2 RPC secret 只能由服务端生成和持有，不暴露给前端。
- FPK 的管理 UI、HTTP API 与 SSE 必须通过 fnOS 统一网关访问，并只接受网关转发的已登录管理员身份。
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
