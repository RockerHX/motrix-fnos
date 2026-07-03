# 飞牛版 Motrix 开发计划

> 本文档记录当前阶段状态、已完成里程碑、优先级和验收标准。长期架构边界见 `docs/architecture.md`；HTTP / SSE 接口见 `docs/api-contract.md`；FPK 构建与产物见 `docs/fpk-packaging.md`；实机验收记录见 `docs/fnos-manual-test-checklist.md`。

## 1. 项目目标

在飞牛 fnOS 上交付一个可安装、可运行、可维护的 **FPK 下载管理应用**。

当前主线：

- 交付形态：FPK + fnOS 服务 + Web UI。
- 后端：Rust server + Axum。
- 前端：Vue 3 + TypeScript + Vite + Naive UI + Pinia。
- 下载引擎：Aria2 Next sidecar。
- 本地持久化：SQLite。
- 通信：HTTP API + SSE。

长期维护范围以 `server/`、`src/`、`packaging/fnos/` 为主。

## 2. 当前状态

更新时间：2026-07-03

当前阶段：**阶段 6：侧栏分类菜单功能化（计划中）**

已完成：

- 前端已切到 HTTP API / SSE 主线，浏览器可通过 `/api/*` 与 `/api/events` 调用后端。
- 后端已落地 Axum API、SSE 事件流、Aria2 进程管理、SQLite 持久化、任务同步和退出收口。
- FPK 目录、manifest、权限配置、Web UI 入口、启动/停止/状态脚本已建立。
- 新建任务保存目录已改为读取 fnOS 已授权目录下拉选择，不再要求用户手动复制路径。
- 打包脚本可输出 x86 与 ARM FPK：
  - `packaging/fnos/dist/motrix.fnos_0.1.0_x86.fpk`
  - `packaging/fnos/dist/motrix.fnos_0.1.0_arm.fpk`
- 已确认 FPK 必须与设备 CPU 架构匹配；x86 包不能安装到 OES / A311D 等 ARM 飞牛设备。
- 飞牛实机已验证安装、启动、停止、状态查询、Web UI、HTTP/HTTPS 下载、暂停、继续、删除、设置、日志和 session 恢复可用。
- 阶段 5 飞牛实机安装和基础功能验证已完成，未发现阻塞问题。

当前约束：

- 阶段 6 优先补齐截图中侧栏菜单的基础可用性，不引入插件运行时、复杂路由或非必要后端重构。
- x86 设备安装 `motrix.fnos_0.1.0_x86.fpk`。
- `aarch64` / `arm64` 设备安装 `motrix.fnos_0.1.0_arm.fpk`。
- 侧栏菜单功能继续遵守 `docs/architecture.md` 的分层边界：Vue 组件只做交互编排，任务数据经 Pinia store / feature service / HTTP API / SSE 流转。

## 3. 阶段里程碑

### 阶段 0：架构纠偏（✅ 已完成）

目标：统一项目方向、文档边界和验收口径，明确 FPK-first 主线。

完成结论：

- 架构文档已收口为 FPK / Rust server / Web UI 模型。
- 开发计划、README、API 契约、打包说明和实机测试清单已建立。
- 后续工作统一围绕 fnOS FPK 交付推进。

状态：✅ 已完成（2026-07-01）。

### 阶段 1：抽出 Rust 业务核心（✅ 已完成）

目标：把 Rust 业务能力收口到独立 server 主线。

完成结论：

- `server/` 已成为 Rust 业务核心承载地。
- `config`、`debug_logs`、`database`、`tasks`、`aria2`、`settings` 等核心能力已进入 server 主线。
- 业务编排已由 service 层承接，后续可在其上继续扩展 HTTP API 与 FPK 运行时能力。

状态：✅ 已完成（2026-07-02）。

### 阶段 2：实现 HTTP API 和事件流（✅ 已完成）

目标：建立 Axum + SSE 的后端服务接口。

完成结论：

- `/api/*` 路由、统一错误响应、设置接口、调试日志接口、任务接口和 Aria2 管理接口已落地。
- `/api/events` 已提供 `tasks.snapshot` 与 `runtime.exiting` 事件。
- 后台任务同步与退出收口已迁入 Tokio runtime。
- server 停止时可广播退出事件、同步任务、保存 Aria2 session，并停止当前管理的 Aria2 实例。

状态：✅ 已完成（2026-07-02）。

### 阶段 3：前端迁移到 HTTP API（✅ 已完成）

目标：让 Vue UI 作为普通 Web UI 运行，消费 HTTP API 与 SSE。

完成结论：

- 前端服务层已切换到 `fetch` + 相对路径 API。
- 运行时事件已切换到浏览器原生 `EventSource`。
- 任务列表刷新主路径已切到 SSE 快照。
- 目录选择、通知、开机自启等系统集成能力已按 Web 安全边界降级。

状态：✅ 已完成（2026-07-02）。

### 阶段 4：建立 FPK 打包链路（✅ 已完成）

目标：生成可用于 fnOS 安装验证的 FPK 产物。

完成结论：

- `packaging/fnos/` 目录、manifest、config、cmd、wizard、图标和 Web UI 入口已建立。
- Rust server、Web UI 静态资源和 Aria2 Next sidecar 已纳入 FPK 组装流程。
- `cmd/start`、`cmd/stop`、`cmd/status` 已建立服务生命周期入口。
- `pnpm run build:fpk` 可执行双架构构建；也可分别执行 `build:fpk:x64` 与 `build:fpk:arm64`。

状态：✅ 已完成（2026-07-02）。

### 阶段 5：飞牛实机安装和基础功能验证（✅ 已完成）

目标：确认最小可用闭环。

已验证：

- 按设备架构构建并安装 FPK。
- 启动服务并打开 Web UI。
- 验证 `/api/app/ping`、任务列表、SSE 刷新和退出态提示。
- 验证 HTTP/HTTPS 下载、暂停、继续、删除。
- 验证设置保存、诊断日志查看与清空。
- 验证停止服务后的 session 保存和重启恢复。

验收标准：

- 匹配架构的 FPK 可安装。
- 应用可启动、停止、查询状态。
- Web UI 可打开并调用后端 API。
- HTTP/HTTPS 下载、暂停、继续、删除可用。
- 设置、日志和 session 恢复可用。
- 卸载无明显残留。

状态：✅ 已完成（2026-07-03）。

### 阶段 6：侧栏分类菜单功能化（计划中）

目标：让侧栏中的 `Downloading`、`Completed`、`Stopped`、`Trash` 和 `Extensions` 都可以选择，并展示与当前架构匹配的基础功能。

现状判断：

- `src/layouts/SidebarNav.vue` 里的分类按钮目前是静态按钮，只有 `Downloading` 写死为 `active`。
- `src/views/MainWindow.vue` 当前直接把全量 `tasks` 传给 `TaskTable`，没有分类状态、筛选结果或分类空态。
- 后端 `TaskService::list_download_tasks` 当前会过滤 `removed` 任务，因此 `Trash` 需要补齐 API / service / persistence 层语义，不能只靠前端筛选。
- 当前架构没有插件运行时，`Extensions` 阶段 6 只做可进入的说明页和后续能力入口，不在本阶段实现真实插件安装、加载或执行。

功能范围：

1. 侧栏导航状态
   - 在 `SidebarNav` 增加当前分类入参和 `selectCategory` 事件。
   - 在 `AppShell` / `MainWindow` 保存当前分类状态，点击菜单后更新高亮。
   - 保持 `Settings`、`Diagnostics` 现有弹窗入口不受影响。
2. 任务分类筛选
   - `Downloading`：展示 `pending`、`active` 任务。
   - `Completed`：展示 `complete` 任务。
   - `Stopped`：展示 `paused`、`error` 任务，保留继续、删除和详情操作。
   - 分类内无任务时展示对应空态，不再统一显示“暂无任务”。
3. Trash 基础能力
   - 后端补充获取已删除任务的能力，例如 `GET /api/tasks?status=removed` 或等价查询参数，并同步更新 `docs/api-contract.md`。
   - `Trash` 展示 `removed` 任务，不混入普通任务列表。
   - 提供最小操作闭环：查看删除记录、永久删除任务记录；是否恢复任务需按 Aria2 GID / 文件存在性设计清楚后再落地。
   - 永久删除只清理 Motrix FNOS 的任务记录；删除用户下载文件必须继续保持显式确认，不做隐式删除。
4. Extensions 基础页
   - 点击 `Extensions` 后进入可见页面，说明当前 FPK Web 版暂未提供插件运行时。
   - 页面预留后续扩展入口，但不接入第三方脚本、不联网拉取插件、不增加新的安全边界。
5. UI 细节
   - 分类菜单可用鼠标点击，当前项高亮与截图风格一致。
   - 可选增加分类计数：下载中、已完成、已停止、回收站数量。
   - 浮动添加按钮只在任务分类页显示，`Extensions` 页不显示。

实施顺序：

1. 前端先实现分类类型、侧栏事件、高亮状态和本地筛选，完成 `Downloading` / `Completed` / `Stopped`。
2. 增加分类空态组件文案，避免不同分类共用不准确的“暂无任务”。
3. 设计并实现 Trash 后端查询 / 永久删除接口，同时更新 `docs/api-contract.md`。
4. 前端接入 Trash 数据源和 Trash 操作按钮。
5. 实现 Extensions 占位页。
6. 做类型检查、后端测试和一次浏览器手动回归。

验收标准：

- 点击 `Downloading`、`Completed`、`Stopped`、`Trash`、`Extensions` 均有响应，当前菜单高亮正确。
- `Downloading` 只显示排队和下载中任务。
- `Completed` 只显示已完成任务，可继续使用重新下载和删除入口。
- `Stopped` 只显示暂停和错误任务，可继续使用恢复和删除入口。
- `Trash` 能看到已删除任务记录，且不污染普通任务列表。
- `Extensions` 有明确页面，不再表现为不可点击。
- 任务新增、暂停、继续、删除、SSE 快照刷新和设置/诊断弹窗不回退。

验证：

- `rtk pnpm run typecheck`
- `rtk cargo test --manifest-path server/Cargo.toml`
- 浏览器手动验证五个侧栏菜单切换、分类筛选、空态、Trash 和 Extensions 页面。

任务追踪：

- [x] 0.1 文档计划提交。
- [x] 1.1 侧栏状态与前三类筛选。
- [ ] 1.2 分类空态与添加按钮规则。
- [ ] 2.1 查询 removed 任务。
- [ ] 2.2 永久删除 removed 记录。
- [ ] 3.1 Trash 页面展示。
- [ ] 3.2 Trash 操作按钮。
- [ ] 4.1 Extensions 基础页。
- [ ] 5.1 全量回归与阶段状态更新。

状态：计划中。

## 4. 当前优先级

1. 先完成前端侧栏分类状态、高亮和 `Downloading` / `Completed` / `Stopped` 筛选。
2. 补齐分类空态和添加按钮显示规则。
3. 设计并实现 Trash 的后端查询 / 永久删除语义，同步更新 `docs/api-contract.md`。
4. 接入 Trash 前端页面和操作按钮。
5. 实现 Extensions 占位页。
6. 完成类型检查、后端测试和浏览器手动回归。

## 5. 验证记录

最近一次文档更新前已确认：

- `rtk pnpm run typecheck` 通过。
- `rtk cargo test --manifest-path server/Cargo.toml` 通过。

阶段 6 预计会修改前端侧栏 / 任务展示组件，并为 Trash 补充任务查询或清理接口；新增或调整接口时必须同步更新 `docs/api-contract.md`。

## 6. 文档关系

- `docs/architecture.md`：长期架构边界。
- `docs/api-contract.md`：前后端接口契约。
- `docs/fpk-packaging.md`：FPK 构建命令、产物路径和排障入口。
- `docs/fnos-manual-test-checklist.md`：阶段 5 实机验收记录模板。
