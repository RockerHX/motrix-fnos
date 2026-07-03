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

当前阶段：**阶段 5：飞牛实机安装和基础功能验证（进行中）**

已完成：

- 前端已切到 HTTP API / SSE 主线，浏览器可通过 `/api/*` 与 `/api/events` 调用后端。
- 后端已落地 Axum API、SSE 事件流、Aria2 进程管理、SQLite 持久化、任务同步和退出收口。
- FPK 目录、manifest、权限配置、Web UI 入口、启动/停止/状态脚本已建立。
- 打包脚本可输出 x86 与 ARM FPK：
  - `packaging/fnos/dist/motrix.fnos_0.1.0_x86.fpk`
  - `packaging/fnos/dist/motrix.fnos_0.1.0_arm.fpk`
- 已确认 FPK 必须与设备 CPU 架构匹配；x86 包不能安装到 OES / A311D 等 ARM 飞牛设备。

未完成：

- 尚未完成真实飞牛设备上的安装、启动、停止、卸载和基础下载闭环验证。
- 尚未完成 HTTP/HTTPS 下载、暂停、继续、删除、设置保存、日志查看和 session 恢复的实机验收。

当前约束：

- 阶段 5 期间优先做实机验证和打包稳定性收尾，不新增非必要功能。
- x86 设备安装 `motrix.fnos_0.1.0_x86.fpk`。
- `aarch64` / `arm64` 设备安装 `motrix.fnos_0.1.0_arm.fpk`。
- 实机失败项优先从 manifest、权限、运行目录、端口、服务脚本和文件夹授权排查。

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

### 阶段 5：飞牛实机安装和基础功能验证（进行中）

目标：确认最小可用闭环。

当前任务：

- 按设备架构构建并安装 FPK。
- 启动服务并打开 Web UI。
- 验证 `/api/app/ping`、任务列表、SSE 刷新和退出态提示。
- 验证 HTTP/HTTPS 下载、暂停、继续、删除。
- 验证设置保存、诊断日志查看与清空。
- 验证停止服务后的 session 保存和重启恢复。
- 验证卸载后端口、进程和运行态文件无明显残留。

验收标准：

- 匹配架构的 FPK 可安装。
- 应用可启动、停止、查询状态。
- Web UI 可打开并调用后端 API。
- HTTP/HTTPS 下载、暂停、继续、删除可用。
- 设置、日志和 session 恢复可用。
- 卸载无明显残留。

状态：进行中。

## 4. 当前优先级

1. 使用匹配架构的 FPK 完成真实飞牛安装验证：
   - x86 设备：`motrix.fnos_0.1.0_x86.fpk`
   - ARM 设备：`motrix.fnos_0.1.0_arm.fpk`
2. 完成启动、停止、状态查询、Web UI、HTTP/HTTPS 下载、暂停、继续、删除、设置保存、日志查看和 session 恢复验证。
3. 完成卸载残留检查。
4. 根据实机失败项修正 FPK manifest、权限、运行目录、端口、服务脚本或文件夹授权，并同步更新手测清单。

## 5. 验证记录

最近一次文档更新前已确认：

- `rtk pnpm run typecheck` 通过。
- `rtk cargo test --manifest-path server/Cargo.toml` 通过。

本次文档更新不新增、不删除、不修改 HTTP API、SSE 事件、环境变量、FPK manifest 字段或构建命令。

## 6. 文档关系

- `docs/architecture.md`：长期架构边界。
- `docs/api-contract.md`：前后端接口契约。
- `docs/fpk-packaging.md`：FPK 构建命令、产物路径和排障入口。
- `docs/fnos-manual-test-checklist.md`：阶段 5 实机验收记录模板。
