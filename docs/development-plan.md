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

更新时间：2026-07-05

当前阶段：**阶段 8：数据目录生命周期与前端缓存策略（计划中）**

已完成：

- 前端已切到 HTTP API / SSE 主线，浏览器可通过 `/api/*` 与 `/api/events` 调用后端。
- 后端已落地 Axum API、SSE 事件流、Aria2 进程管理、SQLite 持久化、任务同步和退出收口。
- FPK 目录、manifest、权限配置、Web UI 入口、启动/停止/状态脚本已建立。
- 新建任务保存目录已改为读取 fnOS 已授权目录下拉选择，不再要求用户手动复制路径。
- 打包脚本可输出 x86 与 ARM FPK：
  - `packaging/fnos/dist/motrix.fnos_0.1.3_x86.fpk`
  - `packaging/fnos/dist/motrix.fnos_0.1.3_arm.fpk`
- 已确认 FPK 必须与设备 CPU 架构匹配；x86 包不能安装到 OES / A311D 等 ARM 飞牛设备。
- 飞牛实机已验证安装、启动、停止、状态查询、Web UI、HTTP/HTTPS 下载、暂停、继续、删除、设置、日志和 session 恢复可用。
- 阶段 5 飞牛实机安装和基础功能验证已完成，未发现阻塞问题。
- 阶段 6 已完成侧栏分类切换、分类空态、Trash 查询与永久删除记录、Extensions 占位页和自动化回归验证。
- 已修复设置页在 fnOS 服务环境缺少 `HOME` 时无法读取默认下载目录的问题；默认下载目录改为从 fnOS 已授权目录选择，优先使用 data 授权目录。
- 阶段 7 已完成设置页能力边界收口，并补齐侧栏 Help 本地帮助入口。
- 0.1.3 已通过前端入口 `/?v=0.1.3` 解决 fnOS WebView 旧前端缓存问题；后续需要把入口版本参数自动化，避免人工漏改。
- 实机发现 `/vol1/@appdata/motrix.fnos/` 在卸载 / 重装相关流程中可能被清空；需要进入阶段 8 查证 fnOS 数据目录生命周期，并补齐升级备份 / 恢复或文档说明。

当前约束：

- x86 设备安装 `motrix.fnos_0.1.3_x86.fpk`。
- `aarch64` / `arm64` 设备安装 `motrix.fnos_0.1.3_arm.fpk`。
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

### 阶段 6：侧栏分类菜单功能化（✅ 已完成）

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
   - 永久删除只清理 Motrix 的任务记录；删除用户下载文件必须继续保持显式确认，不做隐式删除。
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
- [x] 1.2 分类空态与添加按钮规则。
- [x] 2.1 查询 removed 任务。
- [x] 2.2 永久删除 removed 记录。
- [x] 3.1 Trash 页面展示。
- [x] 3.2 Trash 操作按钮。
- [x] 4.1 Extensions 基础页。
- [x] 5.1 全量回归与阶段状态更新。

状态：✅ 已完成（2026-07-03）。

### 阶段 7：设置项功能化与帮助文档入口（✅ 已完成）

目标：把设置页中“真实生效”和“未支持能力”的边界收口清楚，并补齐侧栏 Help 本地入口。

完成结论：

- 设置页保留已生效能力：默认下载目录、最大并发下载数、下载限速、上传限速。
- 设置页已移除开机自启和下载通知入口；前端保存设置时固定写回关闭状态，避免不可用开关误导用户。
- Help 侧栏按钮已接入本地帮助弹窗；内容覆盖授权目录、默认下载目录、下载设置、Trash 永久删除、Extensions 状态和日志诊断入口。
- 诊断弹窗顶部 Aria2 状态已与下方引擎状态面板同步，避免同一弹窗内显示互相矛盾的状态。
- 后端响应已添加 no-cache 头；0.1.3 进一步把 fnOS 桌面入口改为 `/?v=0.1.3`，实机确认可打破 WebView 旧前端缓存。

验收标准：

- Settings 不展示开机自启和下载通知。
- Help 可从侧栏打开和关闭。
- Diagnostics 顶部 Aria2 进程 / RPC 状态与引擎状态面板一致。
- 安装 0.1.3 后能看到新前端 UI，不再停留在旧设置页。

验证：

- `rtk pnpm run typecheck`
- `rtk cargo test --manifest-path server/Cargo.toml`
- `rtk pnpm run build:fpk`
- 实机确认 0.1.3 UI 可见。

状态：✅ 已完成（2026-07-05）。

### 阶段 8：数据目录生命周期与前端缓存策略（计划中）

目标：查清 fnOS FPK 在升级、卸载、重装时对应用数据目录的处理规则，确保 Motrix 的 SQLite、Aria2 session、设置和日志不会在非预期场景丢失；同时把前端入口缓存破坏策略自动化。

现状判断：

- 当前运行数据位于 `TRIM_PKGVAR` 对应目录，实机路径表现为 `/vol1/@appdata/motrix.fnos/`。
- 用户实机观察到 `/vol1/@appdata/motrix.fnos/` 可在卸载 / 重装相关流程后变为空目录，说明必须明确升级与卸载语义。
- 0.1.3 通过手动修改 `packaging/fnos/app/ui/config` 的 `url` 为 `/?v=0.1.3` 解决了旧前端缓存，但该版本参数目前依赖人工同步。

功能范围：

1. 生命周期查证
   - 检查 `cmd/install_*`、`cmd/upgrade_*`、`cmd/uninstall_*`、`cmd/config_*` 是否存在清理数据行为。
   - 查证飞牛官方文档或实机验证 `TRIM_PKGVAR`、`TRIM_PKGHOME`、`TRIM_APPDEST` 在覆盖升级、卸载、重装中的保留策略。
   - 记录查证来源、实机命令和观察结果，不用推断替代结论。
2. 数据保护策略
   - 如果覆盖升级会清空 `TRIM_PKGVAR`，设计升级前备份与升级后恢复，至少覆盖 SQLite、Aria2 session、运行配置和授权目录缓存。
   - 如果只有卸载会清空 `TRIM_PKGVAR`，保持现有目录策略，但在文档中明确“卸载会删除 Motrix 任务记录和设置”。
   - 如 fnOS 提供更适合长期保留的目录，再评估是否迁移数据目录；迁移必须包含兼容旧路径的恢复方案。
3. FPK 生命周期脚本
   - 按查证结果补齐 `upgrade_init` / `upgrade_callback` 的备份、恢复或校验逻辑。
   - 不在卸载流程中额外删除用户下载文件；Motrix 只管理自己的任务记录和运行数据。
   - 增加必要日志，便于实机判断升级前后数据是否被保留或恢复。
4. 前端入口缓存自动化
   - 构建时从 manifest/package 版本生成 `ui/config` 的入口 URL，例如 `/?v=<version>`。
   - 避免后续版本升级时人工漏改导致 fnOS WebView 继续使用旧前端。
   - 保留后端 no-cache 头作为兜底。
5. 文档与验收
   - 更新打包文档和实机测试清单，增加“升级后任务记录/设置/session 是否保留”和“卸载后数据是否清理”的检查项。
   - 明确覆盖安装、卸载重装、重启后安装三种场景的预期结果。

验收标准：

- 覆盖升级后，任务记录、设置、Aria2 session 和日志保留或按设计恢复。
- 卸载后数据目录行为有明确文档说明；如果 fnOS 清空 appdata，则说明这是卸载语义，不误判为升级丢数据。
- 新版本 FPK 的 `ui/config` 自动带版本参数，不需要手动修改。
- 安装新版本后，Help 和设置页能立即展示最新 UI。
- 不删除用户下载文件。

验证：

- `rtk pnpm run typecheck`
- `rtk cargo test --manifest-path server/Cargo.toml`
- `rtk pnpm run build:fpk`
- 解包检查 FPK 中 `ui/config` 的入口 URL 与 manifest 版本一致。
- 实机验证覆盖升级、卸载重装、重启后安装的数据目录行为。

任务追踪：

- [ ] 1.1 查证并记录 fnOS 数据目录生命周期。
- [ ] 2.1 实现升级数据备份 / 恢复或确认无需实现。
- [ ] 3.1 构建时自动同步前端入口版本参数。
- [ ] 4.1 补充打包文档和实机测试清单。
- [ ] 5.1 全量检查、实机回归与阶段状态更新。

状态：计划中。

## 4. 当前优先级

当前优先推进阶段 8：查清 fnOS 应用数据目录在升级、卸载、重装中的保留策略，并把前端入口缓存破坏从人工改版本改为构建自动同步。

## 5. 验证记录

阶段 6 收口已确认：

- `rtk pnpm run typecheck` 通过。
- `rtk cargo test --manifest-path server/Cargo.toml` 通过。
- CLI 环境未执行浏览器手动点击回归；需在下一次实机或浏览器验收中补录五个侧栏菜单切换、分类筛选、空态、Trash 和 Extensions 页面。

阶段 6 已修改前端侧栏 / 任务展示组件，并为 Trash 补充任务查询和永久删除记录接口；接口变更已同步更新 `docs/api-contract.md`。

设置默认下载目录修复已确认：

- `rtk pnpm run typecheck` 通过。
- `rtk cargo test --manifest-path server/Cargo.toml` 通过。
- Rust 单元测试中的 `/tmp`、`/vol1/tmp` 等路径仅用于模拟“已授权 / 未授权目录”和临时测试文件，不代表运行时默认下载目录，也不会写入 FPK 权限配置；运行时目录仍以 fnOS 授权目录列表为准。
- CLI 环境未执行浏览器手动点击回归；需在下一次实机或浏览器验收中补录：无 `HOME` 环境打开设置不报错、默认目录选中 data 授权目录、未授权目录保存失败、新建任务默认保存目录正确。

阶段 7 收口已确认：

- `rtk pnpm run typecheck` 通过。
- `rtk cargo test --manifest-path server/Cargo.toml` 通过。
- `rtk pnpm run build:fpk` 通过。
- 设置页已移除开机自启和下载通知入口，不再表达为已支持的 fnOS 系统能力。
- Help 侧栏入口已接入本地帮助弹窗；帮助内容覆盖授权目录、默认下载目录、下载设置、Trash 永久删除、Extensions 状态和日志诊断入口。
- 实机确认 0.1.3 通过 `/?v=0.1.3` 入口参数打破 fnOS WebView 旧前端缓存，可看到已更新 UI。

## 6. 文档关系

- `docs/architecture.md`：长期架构边界。
- `docs/api-contract.md`：前后端接口契约。
- `docs/fpk-packaging.md`：FPK 构建命令、产物路径和排障入口。
- `docs/fnos-manual-test-checklist.md`：阶段 5 实机验收记录模板。
