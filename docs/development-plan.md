# 飞牛版 Motrix 阶段性开发计划

> 本文档只负责记录阶段目标、具体任务、完成状态、优先级和验收标准。整体技术架构、前后端职责边界、UI 组件策略和目录规范见 `docs/architecture.md`。

> 当前进入 **FPK-first 架构纠偏期**：现有 Tauri 实现仅作为 legacy 资产参考，不再作为最终交付主线。

## 1. 项目目标

本项目的最终目标是在飞牛 fnOS 上交付一个可安装、可运行、可维护的 **FPK 下载管理应用**。

当前确定方向：

- 最终交付形态：**FPK + fnOS 服务 + Web UI**
- 后端主线：**Rust server + Axum**
- 前端主线：**Vue 3 + TypeScript + Vite + Naive UI + Pinia**
- 核心下载引擎：**Aria2 Next sidecar**
- 本地持久化：**SQLite**
- 运行时事件：**HTTP API + SSE（或等价事件流）**

当前仓库中的 Tauri、`src-tauri/`、`@tauri-apps/*` 相关实现仅作为 **legacy 可迁移资产** 保留，用于复用业务逻辑、UI 结构和运行经验，不再代表目标交付路线。

## 2. 产品阶段目标

### 2.1 当前阶段目标

先完成架构纠偏，统一文档和验收口径，避免继续沿 Tauri 桌面模型投入实现成本。

### 2.2 中期阶段目标

完成 Rust 核心抽离、HTTP/SSE 通信改造和前端 Web UI 化，形成可在不启动 Tauri 的前提下运行的服务化下载应用。

### 2.3 最终阶段目标

建立 FPK 打包链路并完成飞牛实机验证，满足安装、启动、停止、卸载和基础下载闭环要求。

## 3. 当前状态摘要

更新时间：2026-07-02

当前阶段：**阶段 3：前端迁移到 HTTP API（进行中）**

已确认的可迁移资产：

- Vue 3 + Naive UI 的任务管理、诊断和设置界面结构。
- Pinia 状态管理与任务轮询/运行态管理模式。
- Rust 中的下载任务模型、Aria2 管理、SQLite 持久化、日志队列和 session 恢复逻辑。
- Linux x86_64 / ARM64 的 Aria2 Next sidecar 资产与下载脚本。

当前主要问题：

- 前端主路径仍存在 `invoke` / `listen` 与 Tauri plugin 直连，阶段 3 需切到 HTTP API / SSE 并完成 Web 降级。
- `src-tauri/` legacy 入口仍需保留到阶段 3 完成后再逐步下线。
- fnOS FPK 目录结构、manifest、cmd 脚本和打包链路仍未建立。
- 数据目录默认值、服务生命周期和前端接口仍保留 legacy 兼容语义，后续阶段需逐步切到 FPK / Web UI 模型。

当前阶段已完成摘要：

- 阶段 0 文档纠偏已完成，主线决策已统一到 FPK-first。
- 阶段 1 已建立 `server/` 核心库，并完成 `config`、`debug_logs`、`database`、`tasks`、纯 `aria2`、`ServerState` 抽离。
- `server::settings::service` 与 `server::tasks::service` 已承接业务编排，`src-tauri` commands 已压薄为 Tauri 适配层。
- 双轨验证通过：`server/` 可独立测试，`src-tauri` 仍可编译并通过现有测试。
- 阶段 2 已建立执行清单、独立 server 入口、server 侧 Aria2 进程管理、Axum 路由骨架，并补齐设置/UI 偏好/调试日志/任务 HTTP 接口。
- `/api/events`、`tasks.snapshot` / `runtime.exiting` SSE 事件流与 Tokio 后台任务同步已落地。
- server 退出流程已具备“广播退出事件 → 同步任务 → 暂停未完成任务并持久化 → 保存 Aria2 session → 停止受管进程 → 成功后清理运行态记录”的收口顺序，阶段 2 验收项已闭环。
- 阶段 3 已启动，当前先补齐前端 HTTP/SSE 迁移清单、Web 降级约定与阶段追踪基线。

当前阶段约束：

- 阶段 2 期间，继续保持 `server/` 与 `src-tauri/` 双轨可运行。
- 在 HTTP API / SSE 替代完成前，不删除现有 Tauri command 和前端调用契约。
- 在 FPK 打包链路建立前，不把 legacy Tauri 启动方式误写为最终交付形态。

## 4. 阶段 0：架构纠偏（✅ 已完成）

### 4.1 目标

先修正文档、阶段定义和验收标准，冻结 Tauri 主线，避免在错误交付模型上继续叠加实现。

### 4.2 已完成小任务

- P0-1：阶段 0 执行清单已建立。✅
- P0-2：架构目标与总体技术路线已切换到 FPK-first。✅
- P0-3：架构分层、数据流和运行时表述已切换到服务化模型。✅
- P0-4：开发计划的目标与现状描述已重置。✅
- P0-5：已新增纠偏阶段并重排后续路线。✅
- P0-6：README 已与整改方向对齐。✅
- P0-7：阶段 0 文档骨架已建立。✅
- P0-8：文档一致性检查与阶段收口已完成。✅

### 4.3 本阶段完成结论

- 主文档已切换到 FPK / Rust server / Web UI 的统一叙述。
- README 已说明当前仍处于整改后待迁移状态，legacy Tauri 链路仅作参考。
- FPK 架构、API 契约、打包说明和实机测试文档骨架已建立。
- 下一阶段可以开始 Rust 核心抽离，但仍不得在没有替代实现前直接删除现有 legacy 资产。

### 4.4 完成标准

- 主文档不再把 Tauri 写成当前交付主线。
- 后续阶段全部以 FPK / Rust server / Web UI 为验收方向。
- README 与新增文档骨架完成后，可进入 Rust 核心抽离阶段。

状态：✅ 已完成（2026-07-01）。

## 5. 后续阶段路线

### 5.1 阶段 1：抽出 Rust 业务核心

目标：把可复用业务从 Tauri command 和运行时胶水中剥离到独立 server 主线。

当前小任务状态：

- P1-1：阶段 1 执行清单已建立。✅
- P1-2：`server/` 核心库 crate 已建立。✅
- P1-3：`config` 与 `debug_logs` 已抽取到 `server/`。✅
- P1-4：`database` 已抽取到 `server/`。✅
- P1-5：`tasks` 领域核心已抽取到 `server/`。✅
- P1-6：纯 `aria2` 核心与 Tauri 进程适配已拆分。✅
- P1-7：`ServerState` 已抽取，`AppState` 已变为 Tauri 适配层。✅
- P1-8：`settings` / `tasks` 服务层已拆分并完成阶段收口。✅

核心任务：

- 建立 `server/` 或等价 Rust crate。
- 迁移 `tasks`、`aria2`、`database`、`debug_logs`、`config`。
- 去掉核心业务对 `tauri::State`、`AppHandle`、`Manager` 的依赖。
- 改造数据目录为 FPK/server config 注入。

验收：

- `cargo test` 可在 server crate 独立运行。
- `src-tauri` 继续可编译并通过现有测试。
- 核心业务不依赖 Tauri。

阶段结论：

- `server/` 已成为 Rust 业务核心承载地，后续可在其上继续引入 HTTP API 与 server 二进制入口。
- `src-tauri` 已退化为 legacy 适配层，保留 Aria2 本地进程管理和 Tauri 运行时胶水。
- 当前已满足阶段 2 启动条件，但仍需维持双轨运行直到 HTTP/SSE 与 Web UI 迁移完成。

状态：✅ 已完成（2026-07-02）。

### 5.2 阶段 2：实现 HTTP API 和事件流

目标：用 Axum + SSE 取代 Tauri command 与事件机制。

当前小任务状态：

- P2-1：阶段 2 执行清单与 API 契约初稿已建立。✅
- P2-2：独立 server 启动入口与运行时配置已建立。✅
- P2-3：server 侧 Aria2 进程管理。✅
- P2-4：Axum 基础接口与统一错误响应。✅
- P2-5：设置与调试日志 HTTP 接口。✅
- P2-6：任务 HTTP 接口与自动拉起 Aria2。✅
- P2-7：SSE 事件流与后台任务同步。✅
- P2-8：优雅关闭与阶段收口。✅

核心任务：

- 建立 `/api/*` 路由。
- 提供统一错误响应。
- 建立 SSE 运行时事件流。
- 把后台任务同步迁移为 Tokio task。

验收：

- 不启动 Tauri 也能通过 HTTP 管理下载任务。
- 服务停止时可保存 session 并停止当前管理的 Aria2。

阶段进展说明：

- 已先锁定阶段 2 的运行时约定：`MOTRIX_FNOS_APP_DATA_DIR`、`MOTRIX_FNOS_HTTP_ADDR`、`MOTRIX_FNOS_ARIA2_PATH`。
- 事件流固定采用 SSE，不引入 WebSocket。
- 首版 SSE 采用“整包任务快照 + 退出事件”模型，避免在阶段 2 提前引入前后端增量同步复杂度。
- `server/src/main.rs`、`ServerRuntimeConfig` 与 `HttpAppState` 已建立，独立 server 主线现在可以完成状态初始化并等待停止信号。
- `server` 已新增 `/api/events`，连接建立后立即推送 `tasks.snapshot`，后台 monitor 每 5 秒同步可见任务变化并广播快照。
- server 停止信号处理已迁入统一 shutdown cleanup，可在退出时广播 `runtime.exiting` 并完成任务/Aria2 收尾。

### 5.3 阶段 3：前端迁移到 HTTP API

目标：把前端主线切到 HTTP + SSE，让 Vue UI 可作为普通 Web UI 运行，同时不新增后端协议、不提前删除 `src-tauri/` Rust legacy 主线。

当前小任务状态：

- P3-1：文档清单与前端迁移矩阵落表。已完成
- P3-2：Web HTTP 基础设施与开发代理。已完成
- P3-3：迁移基础服务到 HTTP。已完成
- P3-4：迁移任务服务并降级目录选择交互。未开始
- P3-5：新增前端 SSE 运行时事件服务。未开始
- P3-6：切换任务刷新主路径到 SSE 快照。未开始
- P3-7：将系统集成功能降级为 Web 安全行为。未开始
- P3-8：清理前端 Tauri 直连依赖并收口阶段 3。未开始

核心任务：

- 新增统一 HTTP client，并把开发态切换到浏览器 + Vite proxy 主线。
- 替换 `invoke` / `listen` 依赖，消费阶段 2 已有 `/api/*` 与 `/api/events`。
- 把任务列表刷新改为“首次拉取 + SSE 快照驱动 + 操作后必要补刷”。
- 清理前端 Tauri 直连依赖，并把系统集成功能降级为纯 Web 安全行为。

验收：

- `pnpm run build` 生成纯 Web 静态资源。
- 浏览器可直接访问 Web UI 并调用后端。
- `src/` 内不再出现 `@tauri-apps/api`、`invoke(`、`listen(`。

阶段进展说明：

- 阶段 3 不新增后端 API，完全复用阶段 2 已落地的 `/api/*` 与 `/api/events` 契约。
- 前端事件流固定采用浏览器原生 `EventSource`，只消费 `tasks.snapshot` 与 `runtime.exiting` 两类事件。
- Web 版系统集成采用“保留并降级”策略：目录选择改为手填；开机自启/通知开关仅保存配置，不再调用宿主插件；不提供 HTTP 版 `quit_app`。
- 阶段收口前继续保持 `server/` 与 `src-tauri/` 双轨可回归。

### 5.4 阶段 4：建立 FPK 打包链路

目标：生成可在飞牛应用中心安装的 `.fpk`。

核心任务：

- 建立 `packaging/fnos/` 目录与 `fnpack` 链路。
- 放入 Rust server 二进制、Web UI `dist/` 和 Linux Aria2 sidecar。
- 编写 `cmd/start`、`cmd/stop`、`cmd/status`。
- 配置 manifest、入口、端口、权限和图标。

验收：

- `fnpack build` 可生成 `.fpk`。
- `.fpk` 可被飞牛应用中心识别安装。

### 5.5 阶段 5：飞牛实机安装和基础功能验证

目标：确认最小可用闭环。

核心任务：

- 安装 `.fpk`。
- 启动服务并打开 Web UI。
- 验证 HTTP/HTTPS 下载、暂停、继续、删除、设置保存、日志查看。
- 验证停止服务后的 session 保存和重启恢复。

验收：

- 基础下载闭环在飞牛上可用。
- FPK 安装、启动、停止、卸载无明显残留。

## 6. Legacy 已有资产与可迁移能力

以下内容仍有迁移价值，但不再代表当前主路线阶段：

- 基于 Tauri 的 Vue + Rust 工程骨架。
- 已完成的任务列表 UI、诊断日志 UI、设置 UI 和 Naive UI / Pinia 分层。
- Rust 侧下载任务模型、Aria2 管理、SQLite 持久化、调试日志与退出清理经验。
- 已验证的 Aria2 Next sidecar 资产管理与多平台下载脚本。

说明：

- 这些成果视为可迁移资产，不再作为“继续补完 Tauri 应用”路线推进。
- 如需查阅历史行为或迁移来源，以现有代码和提交记录为准，不再把旧阶段文档作为未来实施主线。

## 7. 当前优先级

1. 推进阶段 3：把前端服务层、任务流和运行时事件从 `invoke` / `listen` 迁到 HTTP / SSE。
2. 在不破坏双轨运行的前提下，完成 Web 降级与前端 Tauri 直连依赖清理。
3. 待浏览器主线稳定后，再推进 FPK 打包链路和飞牛实机验证。

## 8. 验收原则

- 优先验证方向是否正确，再验证实现是否完整。
- 阶段 0 只做文档与验收口径治理，不把代码迁移混入其中。
- 每个后续阶段都必须有可独立验证的交付物，不允许长期停留在“半迁移状态”。
- 若架构文档、开发计划和整改计划冲突，以 `docs/architecture.md` 的长期边界和整改计划的当前优先级为准。

## 9. 总体判断

当前项目并非全部作废，而是需要把已经积累的前端和 Rust 业务资产从 Tauri 主线中抽离出来，转向 FPK-first 的服务化交付模型。

阶段 0 已完成文档纠偏，阶段 1 也已完成 Rust 核心抽离。接下来的关键不是继续堆叠 Tauri 能力，而是围绕 `server/` 主线推进 HTTP API、Web UI 和 FPK 交付闭环。
