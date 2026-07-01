# 飞牛版 Motrix 架构文档

> 本文档负责约束项目的整体技术架构、前后端职责边界、UI 组件策略和目录组织。阶段任务、状态和优先级见 `docs/development-plan.md`。

## 1. 架构目标

飞牛版 Motrix 的目标不是继续演进为桌面应用，而是交付一个可在飞牛 fnOS 中安装、启动、停止、升级和卸载的 **FPK-first 下载管理应用**。

当前长期架构必须满足以下目标：

- **可交付**：最终以 `.fpk` 形式交付，并符合 fnOS 应用中心的运行模型。
- **可运行**：以 Rust 后端服务 + Vue Web UI 的形态运行，而不是依赖桌面窗口语义。
- **可维护**：前端、后端、下载引擎、持久化、日志各自边界清晰，避免单文件堆叠。
- **可恢复**：任务、配置、历史状态最终必须可持久化，支持应用重启后恢复。
- **可排障**：生产环境不能依赖开发终端，必须有应用内日志和诊断入口。
- **可扩展**：后续能自然扩展 BT、磁力、批量任务、远程管理和自动化能力。

## 2. FPK-first 总体技术路线

当前项目的目标主线采用以下技术路线：

- 交付形态：**fnOS FPK 应用包**
- 后端主程序：**Rust server**
- 服务框架：**Axum**
- 下载引擎：**Aria2 Next sidecar**
- 前端框架：**Vue 3 + TypeScript + Vite**
- UI 组件库：**Naive UI**
- 前端状态管理：**Pinia**
- 本地持久化：**SQLite**
- Rust 异步运行时：**Tokio**
- Rust 序列化：**Serde**
- Rust 数据库访问：**SQLx**
- 日志与诊断：**tracing + 应用内调试日志队列**
- 前后端通信：**HTTP API + SSE（或等价事件流）**

当前仓库中的 Tauri 代码、脚本和目录仅视为 **legacy 迁移来源**：

- 可复用其中的业务逻辑、前端 UI、状态管理、Aria2 管理和 SQLite 资产。
- 不再把 Tauri 2 写作当前正式应用壳。
- 不再把 `tauri build`、窗口/托盘能力或 Tauri command 作为目标交付路线。

## 3. 总体目标架构

整改后的目标架构如下：

```text
fnOS FPK
  ├─ manifest / config / cmd / wizard / icons
  ├─ Rust server
  │   ├─ Axum HTTP API
  │   ├─ SSE 事件流
  │   ├─ Aria2 Next 进程管理
  │   ├─ SQLite 持久化
  │   └─ 调试日志与运行时状态
  ├─ Web UI
  │   ├─ Vue 3 + Naive UI + Pinia
  │   ├─ HTTP API client
  │   └─ 浏览器 / fnOS Web 入口
  └─ Aria2 Next Linux sidecar
```

本架构下的重点不是“窗口能否打开”，而是：

- fnOS 是否能正确启动和停止服务。
- Web UI 是否能通过 API 管理下载任务。
- Aria2 和 SQLite 状态是否能在服务生命周期内稳定恢复。

## 4. 分层职责

### 4.1 FPK 打包层

FPK 打包层负责 fnOS 应用安装与运行入口。

职责：

- 提供 manifest、权限、图标、安装信息和 Web 入口配置。
- 负责 start / stop / status 等服务控制脚本。
- 负责把 Rust server、Web UI 静态资源和 Aria2 sidecar 打包进 FPK。

不职责：

- 下载业务本身。
- 页面展示与交互。
- 直接持久化业务数据。

### 4.2 Rust Server 核心层

Rust 负责核心业务能力，不负责页面展示。

职责：

- 业务模型定义。
- 下载任务生命周期管理。
- 配置读取与校验。
- 下载目录、文件路径、安全边界处理。
- 启动、停止、监控 Aria2 Next sidecar。
- 与 Aria2 JSON-RPC 通信。
- SQLite 持久化。
- 应用内日志写入。
- 暴露 HTTP API 和运行时事件流给前端调用。

不职责：

- 页面布局。
- 表格渲染。
- 组件交互。
- 前端临时状态管理。

### 4.3 Aria2 Next 下载引擎层

Aria2 Next 只负责真实下载能力。

职责：

- HTTP / HTTPS / BT / 磁力等底层下载。
- 分片、并发、断点续传、限速等下载能力。
- 通过 JSON-RPC 接受任务控制。
- 返回任务状态、进度、速度、错误码和文件信息。

不职责：

- 任务列表 UI。
- 应用配置存储。
- 历史记录存储。
- fnOS 应用生命周期控制。

### 4.4 Vue Web UI 层

Vue 负责 UI 展示和用户交互，不直接承载后端业务规则。

职责：

- 主页面布局。
- 任务列表、任务详情、新建任务、设置、诊断日志等页面。
- 用户输入校验中的轻量即时反馈。
- 调用 service / store 完成交互。
- 表格、弹窗、Toast、菜单等交互组件。

不职责：

- 真实下载逻辑。
- 文件系统权限判断。
- 持久化数据库读写。
- 直接拼装复杂 Aria2 RPC 请求。

### 4.5 Pinia 状态层

Pinia 负责前端全局和跨组件状态。

职责：

- 任务列表状态。
- 当前筛选条件。
- 当前选中任务。
- 任务轮询或事件订阅状态。
- 配置缓存。
- UI 偏好，例如表格列宽、当前分类、侧栏展开状态。

不职责：

- SQLite 持久化。
- 直接发起底层 Aria2 RPC。
- 复杂后端业务判断。

### 4.6 SQLite 持久化层

SQLite 是本地长期状态来源。

职责：

- 下载任务记录。
- 配置项。
- 历史任务。
- 错误记录。
- 需要长期保存的 UI 偏好。

不职责：

- 当前页面临时状态。
- 实时渲染。
- Aria2 实时下载执行。

## 5. 前端架构规范

### 5.1 目录结构

目标前端目录结构如下：

```text
src/
  app/
    providers/
      NaiveProvider.vue
  layouts/
    AppShell.vue
    SidebarNav.vue
    Topbar.vue
  views/
    MainWindow.vue
  features/
    tasks/
      components/
      stores/
      composables/
      services/
      types.ts
    diagnostics/
      components/
      stores/
      services/
    settings/
      components/
      stores/
      services/
  services/
    http.ts
    runtimeEvents.ts
  types/
    app.ts
    aria2.ts
```

说明：

- `views/` 只放页面入口，负责组合布局和功能模块。
- `layouts/` 放通用页面结构，例如侧栏、顶部栏、整体 shell。
- `features/` 按业务领域拆分，任务、日志、设置等都应进入各自 feature。
- `services/` 放 HTTP client 和运行时事件订阅封装。
- `MainWindow.vue` 当前仍可作为 legacy 文件名保留，但语义上视为 Web UI 页面入口，而不是桌面窗口控制器。

### 5.2 `MainWindow.vue` 边界

`MainWindow.vue` 只允许承担页面编排职责。

允许：

- 引入布局组件。
- 引入任务模块组件。
- 引入诊断模块组件。
- 处理极少量页面级开关状态。

不允许：

- 直接实现完整任务表。
- 直接实现复杂弹窗表单。
- 直接实现 Toast 队列。
- 直接实现任务轮询或事件流管理。
- 直接调用后端接口而绕过 feature service / store。

## 6. UI 组件策略

### 6.1 正式采用 Naive UI

项目正式采用 **Naive UI** 作为 Vue UI 组件库。

原因：

- 与 Vue 3 / TypeScript 适配成熟。
- 提供 DataTable、Dialog、Message、Notification、Form、Input、Button、Tabs 等基础能力。
- 可以减少自研控件带来的交互和可维护性成本。
- 适合先快速做出稳定工具，再逐步定制视觉风格。

### 6.2 组件库使用原则

优先使用 Naive UI：

- 表格：`NDataTable`
- 弹窗：`NModal` / `NDialog`
- 表单：`NForm` / `NInput` / `NSelect`
- 按钮：`NButton`
- Tabs：`NTabs`
- Toast / 提示：`NMessage` / `NNotification`
- 进度：`NProgress`
- 空状态：`NEmpty`

允许自定义 CSS：

- 应用整体暗色主题。
- 飞牛 / Motrix 风格的颜色、间距、圆角。
- 侧栏和整体 shell 的产品化布局。

## 7. 后端目录与模块约束

目标后端结构如下：

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
    db/
    logs/
```

约束：

- `api/` 只负责 HTTP handler 和请求/响应转换。
- `services/` 负责业务流程编排。
- `tasks/`、`aria2/`、`config/`、`db/`、`logs/` 保持清晰边界。
- 现有 `src-tauri/` 目录在迁移完成前仅作为 legacy 参考来源，不再承载长期架构决策。

## 8. 标准数据流与事件流

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

标准事件流：

```text
Rust Runtime Event
  -> SSE
  -> frontend runtime event service
  -> Pinia Store
  -> Components
```

禁止：

- Vue 组件直接散落调用后端接口。
- 前端直接拼装复杂 Aria2 RPC 请求。
- Rust handler 内直接堆积业务逻辑而不拆 service。
- 把 UI 临时状态和后端持久状态混成同一层对象。

## 9. 运行时生命周期原则

当前长期运行模型基于 fnOS 服务，而不是桌面窗口或托盘语义。

固定原则：

- 应用启动与停止以 fnOS 的 start / stop / status 语义为准。
- 后端启动后负责准备数据目录、初始化 SQLite、启动或连接 Aria2。
- 后端停止时应统一保存运行状态、刷新必要持久化并停止当前服务管理的 Aria2 实例。
- 前端页面关闭、刷新或重新进入不应被视为应用退出。
- 不再把 Dock、托盘、窗口隐藏或桌面通知写成长期固定原则；如未来需要兼容，只能作为 legacy 支持或附加能力。

## 10. 数据目录与安全边界

- SQLite、Aria2 session、Aria2 log 和运行态文件必须放在 FPK 应用数据目录。
- 数据目录优先从 fnOS / FPK 提供的环境或配置读取，不再依赖 Tauri app data dir。
- 下载目录不能默认写死桌面用户目录；必须改为 fnOS 可访问目录或应用数据目录下的默认下载区。
- Aria2 RPC secret 只能在服务端生成和持有，不对前端暴露。
- 日志继续隐藏私密 URL query 和敏感配置。

## 11. 开发约束

后续开发必须遵守：

- 不再新增 Tauri 主线能力。
- 新增前端交互继续进入 `features/*`，不得重新向入口页面堆叠。
- 新增后端能力必须按 `api -> service -> domain -> persistence` 分层。
- 新增通信能力默认走 HTTP API / SSE，不再新增 Tauri command。
- Tauri 相关目录、脚本和配置在迁移完成前仅作为 legacy 参考，不得继续扩展为目标主线。
- 新增长期状态时必须考虑 SQLite 持久化路径。
- 若后续发现本文档与实际演进不匹配，应先更新本文档，再继续实现。
