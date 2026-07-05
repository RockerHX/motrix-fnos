# 飞牛版 Motrix 开发计划

> 更新时间：2026-07-05  
> 本文档记录当前阶段状态、已完成里程碑、优先级和验收口径。长期架构边界见 `docs/architecture.md`；HTTP / SSE 与 JSON-RPC 接口见 `docs/api-contract.md`；FPK 构建与产物见 `docs/fpk-packaging.md`。

## 1. 项目目标

在飞牛 fnOS 上交付一个可安装、可运行、可维护的 **FPK 下载管理应用**。

当前主线：

- 交付形态：FPK + fnOS 服务 + Web UI。
- 后端：Rust server + Axum。
- 前端：Vue 3 + TypeScript + Vite + Naive UI + Pinia。
- 下载引擎：Aria2 Next sidecar。
- 本地持久化：SQLite。
- 通信：HTTP API + SSE；公网/解析站兼容入口使用 `/jsonrpc`。
- 长期维护范围：`server/`、`src/`、`packaging/fnos/`。

## 2. 当前状态

当前版本：**1.2.0**  
当前阶段：**阶段 10：手机端 UI 适配（✅ 已完成）**

已完成能力：

- Rust server 已落地 Axum API、SSE 事件流、Aria2 进程管理、SQLite 持久化、任务同步、日志诊断和退出收口。
- Web UI 已切换到相对路径 HTTP API / SSE 主线，支持任务列表、分类侧栏、回收站、Extensions 占位页、设置、帮助、诊断日志和应用内中英文切换。
- 新建任务保存目录来自 fnOS 已授权目录，不提供任意本地路径输入。
- FPK 目录、manifest、权限配置、Web UI 入口、启动/停止/状态脚本和卸载私有数据清理脚本已建立。
- 打包脚本可输出 x86 与 ARM FPK：
  - `packaging/fnos/dist/motrix.fnos_1.2.0_x86.fpk`
  - `packaging/fnos/dist/motrix.fnos_1.2.0_arm.fpk`
- 飞牛实机已验证安装、启动、停止、状态查询、Web UI、HTTP/HTTPS 下载、暂停、继续、删除、设置、日志和 session 恢复可用。
- JSON-RPC 兼容入口已支持 `aria2.addUri`、`aria2.getVersion` 和 `system.multicall`，并通过设置页的 `jsonRpcToken` 控制添加任务鉴权。
- 手机端 UI 适配本轮开发已完成，覆盖移动端外壳布局、任务卡片、空态、创建任务、设置、帮助、诊断与日志弹窗。

当前约束：

- `x86_64` 设备安装 `motrix.fnos_1.2.0_x86.fpk`。
- `aarch64` / `arm64` 设备安装 `motrix.fnos_1.2.0_arm.fpk`。
- FPK 必须与设备 CPU 架构匹配；x86 包不能安装到 ARM 飞牛设备。
- 桌面 Web、手机浏览器和飞牛 App WebView 共用同一套 Vue 源码、Pinia store、service、HTTP API 和 SSE 数据流。
- 侧栏菜单、移动端展示和后续新增前端交互继续遵守 `docs/architecture.md` 的分层边界。

## 3. 已完成里程碑

| 阶段 | 状态 | 完成结论 |
| --- | --- | --- |
| 阶段 0：架构纠偏 | ✅ 2026-07-01 | 收口为 FPK / Rust server / Vue Web UI / Aria2 sidecar 主线，并建立架构、计划、API 和打包文档边界。 |
| 阶段 1：抽出 Rust 业务核心 | ✅ 2026-07-02 | `server/` 成为业务核心承载地，配置、日志、数据库、任务、Aria2 和设置能力进入 server 主线。 |
| 阶段 2：HTTP API 和事件流 | ✅ 2026-07-02 | `/api/*`、统一错误响应、任务/设置/日志/Aria2 接口和 `/api/events` SSE 已落地。 |
| 阶段 3：前端迁移到 HTTP API | ✅ 2026-07-02 | Vue UI 作为普通 Web UI 运行，通过 `fetch` 与 `EventSource` 消费后端。 |
| 阶段 4：FPK 打包链路 | ✅ 2026-07-02 | `packaging/fnos/`、服务脚本、静态资源、server 和 Aria2 sidecar 已纳入组装流程。 |
| 阶段 5：飞牛实机验证 | ✅ 2026-07-03 | 匹配架构的 FPK 可安装运行，核心下载、设置、日志、停止和 session 恢复可用。 |
| 阶段 6：侧栏分类菜单功能化 | ✅ 2026-07-03 | Downloading / Completed / Stopped / Trash / Extensions 可切换，Trash 查询和永久删除记录已完成。 |
| 阶段 7：设置项与帮助入口 | ✅ 2026-07-05 | 设置页只保留已生效能力，帮助弹窗和诊断状态同步已补齐，前端缓存策略通过入口版本参数与 no-cache 响应头收口。 |
| 阶段 8：数据目录生命周期 | ✅ 2026-07-05 | 升级不主动清理私有数据；卸载只清理 `TRIM_PKGVAR` 下的 Motrix 私有运行数据，不删除用户下载文件。 |
| 阶段 9：应用内国际化 | ✅ 2026-07-05 | Web UI 支持简体中文 / English 手动切换并通过设置持久化；平台外壳保持中文。 |
| 阶段 10：手机端 UI 适配 | ✅ 2026-07-05 | 移动端切换为单列外壳、底部导航、任务卡片和移动端弹窗；桌面布局保持不回退。 |

## 4. 当前优先级

当前没有新的进行中阶段。后续工作应按需求单独立项，并先确认是否影响以下边界：

- 是否新增或改变 HTTP / SSE / JSON-RPC 契约。
- 是否新增长期状态、数据库字段或迁移。
- 是否涉及 fnOS / FPK 生命周期、权限、端口入口或文件夹授权行为。
- 是否需要更新 `docs/architecture.md` 的长期架构约束。

## 5. 验证口径

常规自动验证：

```bash
rtk pnpm run verify:pre-commit
```

发布前验证：

```bash
rtk pnpm run verify
rtk pnpm run build:fpk
```

实机重点验证：

- 按设备架构安装对应 FPK。
- 启动服务并打开 Web UI。
- 验证 `/api/app/ping`、任务列表、SSE 刷新和退出态提示。
- 验证 HTTP/HTTPS 下载、暂停、继续、删除、回收站和永久删除记录。
- 验证设置保存、JSON-RPC token、诊断日志查看与清空。
- 验证停止服务后的 session 保存和重启恢复。
- 验证手机端首屏、分类导航、创建任务、任务卡片、设置、帮助、诊断和日志弹窗。

## 6. 文档关系

- `docs/architecture.md`：长期架构边界和分层约束。
- `docs/api-contract.md`：HTTP / SSE / JSON-RPC 接口契约。
- `docs/fpk-packaging.md`：FPK 构建命令、产物路径和排障入口。
- `docs/mobile-ui-adaptation-plan.md`：手机端 UI 适配完成记录。
- `CHANGELOG.md`：发布历史。
