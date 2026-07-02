# Motrix FNOS FPK-first 整改计划

> 生成时间：2026-07-01  
> 目的：纠正当前项目从“飞牛 fnOS 可安装 FPK 应用”偏向“Tauri 桌面应用”的架构偏差，明确哪些代码保留、哪些代码重构、哪些代码删除，以及哪些文档必须更新。

## 1. 结论摘要

当前项目**已经严重偏离最初目标的交付形态**。

最初目标是：

- 在飞牛 fnOS 上安装。
- 以 `.fpk` 形式交付。
- 通过飞牛应用中心启动、停止、卸载和访问。
- 适配 NAS 应用运行模型。

当前实际实现是：

- 使用 **Tauri 2** 作为主应用壳。
- 以前端 `@tauri-apps/api` 调用 Rust `tauri::command`。
- 依赖 Tauri 窗口、托盘、系统通知、系统目录选择、Tauri sidecar 打包。
- 通过 `tauri.conf.json` 打包桌面应用，而不是通过 `fnpack` 生成 FPK。

这意味着当前项目不是“差最后一步打包 FPK”，而是**主运行模型选错了**。

不过，项目并非全部作废。业务内核仍有较高复用价值：

- Vue 3 + Naive UI 的任务管理界面可保留并改造成 Web UI。
- Rust 中的 Aria2 管理、任务模型、SQLite、日志、下载生命周期逻辑大部分可保留。
- Aria2 Next Linux sidecar 资产和 session 恢复逻辑可保留。

必须重构的是：

- 应用壳：从 Tauri 桌面壳改为 FPK 安装包 + Rust 后端服务 + Web UI。
- 通信层：从 Tauri invoke/event 改为 HTTP API + WebSocket/SSE。
- 生命周期：从窗口/托盘/Dock 语义改为 fnOS 应用中心 start/stop/status 语义。
- 打包链路：从 `tauri build` 改为 `fnpack build`。

整改执行原则：**文档先行，代码随后**。在 `docs/architecture.md`、`docs/development-plan.md` 和 `README.md` 完成 FPK-first 纠偏前，不应继续迁移代码、删除 Tauri 模块或新增功能。否则旧文档会继续把后续开发牵回错误方向。

## 2. 官方 FPK 方向依据

飞牛应用开放平台文档显示，fnOS 应用开发围绕以下概念：

- FPK 通过 `fnpack` 创建项目结构和打包为可安装 `.fpk` 文件。
- 应用目录包含 `manifest`、`config/`、`cmd/`、`wizard/`、图标和资源。
- 应用可有 Web 访问入口，也可创建纯服务类型。
- `manifest` 中存在应用入口、监听端口、启动/停止控制等配置。
- fnOS 基于 Linux/Debian 运行环境，服务端可使用 Linux 支持的语言，前端可使用 HTML/CSS/JavaScript 或现代前端框架。

参考入口：

- `https://developer.fnnas.com/docs/guide`
- `https://developer.fnnas.com/docs/cli/fnpack`
- `https://developer.fnnas.com/docs/core-concepts/framework`
- `https://developer.fnnas.com/docs/core-concepts/manifest`

因此，本项目后续必须以 **FPK-first** 为唯一主线。

## 3. 偏离程度评估

### 3.1 偏离等级

偏离等级：**高**

原因：

- 项目最外层工程、脚本、依赖和文档都以 Tauri 为主。
- Rust 入口强绑定 `tauri::Builder`。
- 前端服务层强绑定 `@tauri-apps/api`。
- 生命周期设计围绕窗口隐藏、托盘退出、Dock 重新打开。
- 当前没有 `fpk/`、`packaging/fnos/`、`manifest`、`cmd/start`、`cmd/stop` 或 `fnpack` 打包链路。

### 3.2 影响范围

| 范围 | 当前状态 | 整改判断 |
| --- | --- | --- |
| 前端 UI | Vue + Naive UI，可在浏览器中运行 | 大部分保留 |
| 前端通信 | Tauri invoke/event | 必须重写 |
| Rust 业务逻辑 | 下载任务、Aria2、SQLite、日志 | 大部分保留 |
| Rust 应用入口 | Tauri Builder | 必须重写 |
| 生命周期 | 桌面窗口/托盘模型 | 必须重写 |
| 系统能力 | Tauri dialog/autostart/notification/opener | 必须替换或删除 |
| 打包 | Tauri bundle | 必须删除主线，改为 fnpack |
| 文档 | 仍宣称 Tauri 是主路线 | 必须更新 |

## 4. 目标架构

整改后的目标架构：

```text
fnOS FPK
  ├─ manifest / config / cmd / wizard / icons
  ├─ Rust 后端服务（motrix-fnos-server）
  │   ├─ HTTP API（Axum）
  │   ├─ WebSocket 或 SSE 事件流
  │   ├─ Aria2 Next 进程管理
  │   ├─ SQLite 持久化
  │   ├─ 调试日志
  │   └─ 统一启动/停止清理
  ├─ Web UI（Vue 3 + Naive UI + Pinia）
  │   ├─ 静态资源 dist/
  │   ├─ HTTP API client
  │   └─ 浏览器/fnOS 应用入口
  └─ Aria2 Next Linux sidecar
      ├─ x86_64-unknown-linux-gnu
      └─ aarch64-unknown-linux-gnu
```

标准数据流改为：

```text
Vue Component
  -> Pinia Store
  -> Feature Service
  -> HTTP client
  -> Axum Route
  -> Rust Service / Repository
  -> Aria2 JSON-RPC / SQLite
```

标准事件流改为：

```text
Rust Runtime Event
  -> WebSocket/SSE
  -> Frontend runtime event service
  -> Pinia Store
  -> Components
```

## 5. 现有代码模块审计

### 5.1 必须保留的模块

这些模块和目标 FPK 架构一致，后续应尽量保留并只做接口层适配。

#### 前端 UI 与状态

| 路径 | 判断 | 说明 |
| --- | --- | --- |
| `src/features/tasks/components/` | 保留 | 任务表、创建弹窗、状态展示仍适用于 Web UI |
| `src/features/tasks/stores/taskStore.ts` | 保留但改通信依赖 | Pinia 状态模型可复用 |
| `src/features/tasks/composables/useTaskPolling.ts` | 保留但改退出事件来源 | 轮询机制可复用，后续可改成 SSE/WebSocket |
| `src/features/diagnostics/` | 保留 | 应用内日志能力仍必要 |
| `src/features/settings/` | 保留但改功能项 | 设置 UI 可复用，开机自启/通知等字段需重判 |
| `src/layouts/` | 保留 | Web UI 布局可复用 |
| `src/views/MainWindow.vue` | 保留但改名候选 | 后续可改为 `AppView.vue` 或 `DashboardView.vue` |
| `src/app/providers/NaiveProvider.vue` | 保留 | UI provider 仍适用 |
| `src/types/` | 保留但同步 API 类型 | 类型定义可复用 |

#### Rust 业务内核

| 路径 | 判断 | 说明 |
| --- | --- | --- |
| `src-tauri/src/tasks/mod.rs` | 保留并迁移到 server crate | 任务模型、校验、Aria2 任务转换、session 匹配逻辑有价值 |
| `src-tauri/src/aria2/mod.rs` | 保留但拆分 | Aria2 RPC、sidecar 启停、session、端口选择逻辑可复用 |
| `src-tauri/src/config/aria2.rs` | 保留并迁移 | Aria2 配置仍需要 |
| `src-tauri/src/database/` | 保留但去 Tauri path 依赖 | SQLite schema/repository 可复用 |
| `src-tauri/src/debug_logs/` | 保留 | FPK 生产排障仍需要 |
| `src-tauri/src/commands/settings.rs` 中配置结构和规范化函数 | 部分保留 | Tauri command 注解删除，业务函数保留 |
| `src-tauri/src/commands/tasks.rs` 中任务控制流程 | 部分保留 | command 层删除，内部流程迁移为 service/handler |

#### 资产和脚本

| 路径 | 判断 | 说明 |
| --- | --- | --- |
| `src-tauri/binaries/aria2-next-x86_64-unknown-linux-gnu` | 保留 | 飞牛 x86_64 目标需要 |
| `src-tauri/binaries/aria2-next-aarch64-unknown-linux-gnu` | 保留 | 飞牛 ARM64 目标候选 |
| `scripts/fetch-aria2-next.mjs` | 保留但改输出路径 | 可继续管理 Aria2 Next 资产 |
| `.github/workflows/verify.yml` | 保留但重写 | CI 仍需要，但验证目标要改为 server + web + fpk |

### 5.2 必须重构的模块

这些模块不能直接用于 FPK，但内部部分逻辑可迁移。

#### Rust Tauri command 层

| 路径 | 问题 | 整改方式 |
| --- | --- | --- |
| `src-tauri/src/commands/app.rs` | Tauri command 入口 | 改成 Axum `/api/app/info`、`/api/app/ping`、`/api/app/quit` 或服务控制接口 |
| `src-tauri/src/commands/aria2.rs` | Tauri command 入口 | 改成 Axum `/api/aria2/*` |
| `src-tauri/src/commands/debug_logs.rs` | Tauri command 入口 | 改成 Axum `/api/debug-logs` |
| `src-tauri/src/commands/settings.rs` | Tauri command + 部分业务混合 | 拆成 `settings/service.rs` + `api/settings.rs` |
| `src-tauri/src/commands/tasks.rs` | Tauri command + 业务流程混合 | 拆成 `tasks/service.rs` + `api/tasks.rs` |
| `src-tauri/src/commands/mod.rs` | Tauri commands 聚合 | 删除或替换为 `api/mod.rs` |

#### Rust 应用入口与运行时

| 路径 | 问题 | 整改方式 |
| --- | --- | --- |
| `src-tauri/src/lib.rs` | 强绑定 Tauri Builder、窗口、托盘、菜单、事件 | 重新开发为 server bootstrap；原业务函数按需搬迁 |
| `src-tauri/src/main.rs` | 调用 Tauri lib run | 改为启动 Rust server |
| `src-tauri/src/runtime/mod.rs` | 使用 Tauri AppHandle 和通知插件 | 改成后台 monitor + WebSocket/SSE 事件；系统通知先删除或后续接 fnOS 能力 |
| `src-tauri/src/app/mod.rs` | AppState 可复用但路径和生命周期有 Tauri 语义 | 改成 `ServerState`；数据目录从环境变量/FPK 路径推导 |
| `src-tauri/src/database/mod.rs` | `database_path(app: &tauri::AppHandle)` 强绑定 Tauri path | 改为 `database_path_from_env()` 或从 server config 注入 |

#### 前端服务层

| 路径 | 问题 | 整改方式 |
| --- | --- | --- |
| `src/services/backend.ts` | 使用 Tauri `invoke` | 改为 HTTP fetch |
| `src/services/aria2.ts` | 使用 Tauri `invoke` | 改为 HTTP fetch |
| `src/services/settings.ts` | 使用 Tauri `invoke` | 改为 HTTP fetch |
| `src/services/runtime.ts` | 使用 Tauri `listen` | 改为 SSE/WebSocket 订阅 |
| `src/features/tasks/services/taskService.ts` | 使用 Tauri `invoke` | 改为 HTTP fetch |
| `src/features/diagnostics/services/debugLogService.ts` | 使用 Tauri `invoke` | 改为 HTTP fetch |
| `src/features/settings/services/systemIntegrationService.ts` | 使用 Tauri autostart/notification API | 删除或改为 fnOS 后端能力 |

#### 前端组件中的桌面语义

| 路径 | 问题 | 整改方式 |
| --- | --- | --- |
| `src/features/settings/components/SettingsDialog.vue` | “后台驻留”“开机自启”“下载通知”是 Tauri 桌面语义 | 改成 fnOS 服务语义：随应用启动、下载目录、限速、并发、日志等 |
| `src/components/EngineStatusPanel.vue` | 需要确认是否展示 Tauri 进程语义 | 保留 UI，改成后端服务/Aria2 状态 |
| `src/views/MainWindow.vue` | 页面名受桌面窗口影响 | 可保留功能，后续重命名 |

### 5.3 必须删除或降级为非主线的模块

这些模块不应进入 FPK 主线。

| 路径 | 处理 | 原因 |
| --- | --- | --- |
| `src-tauri/tauri.conf.json` | 删除或移动到 `legacy/tauri/` | FPK 不使用 Tauri bundle |
| `src-tauri/build.rs` | 删除 | Tauri build 脚本 |
| `src-tauri/capabilities/` | 删除 | Tauri 权限配置 |
| `src-tauri/icons/*.icns`、`*.ico`、Windows/MS Store 图标 | 删除 | FPK 只需要 fnOS 规定图标资源 |
| `src-tauri/icons/Square*.png`、`StoreLogo.png` | 删除 | Tauri/Windows 包资源 |
| `src-tauri/binaries/aria2-next-aarch64-apple-darwin` | 删除或移入开发资产 | 飞牛不需要 macOS sidecar |
| `scripts/tauri-dev.mjs` | 删除或移动到 legacy | Tauri dev 非主线 |
| `package.json` 中 `tauri`、`tauri:dev`、`tauri:build` 脚本 | 删除 | 防止继续走错打包路线 |
| `package.json` 中 `@tauri-apps/*` 依赖 | 删除 | 前端改为 HTTP/WebSocket |
| `src/assets/vue.svg`、`public/tauri.svg`、`public/vite.svg` | 删除 | 脚手架残留 |
| `src-tauri/src/aria2/mod.rs.tmp` | 删除 | 空临时文件 |
| 本地 `.DS_Store` 文件 | 不纳入仓库；若已跟踪则删除 | macOS 垃圾文件 |

注意：删除 Tauri 主线前必须先完成 server/API 迁移，避免中间态完全不可运行。

## 6. 当前缺失模块

为了成为可安装 FPK，当前至少缺失以下模块。

### 6.1 FPK 包结构

建议新增：

```text
packaging/fnos/
  manifest
  ICON.PNG
  ICON_256.PNG
  config/
    resource
    permission
  cmd/
    install_callback
    uninstall_callback
    start
    stop
    status
  ui/
    config
    dist/
  app/
    bin/
      motrix-fnos-server
      aria2-next
    data/
```

实际文件名和字段以 `fnpack create` 生成结果及官方文档为准。

### 6.2 Rust server

建议新增或重组：

```text
server/
  Cargo.toml
  src/
    main.rs
    state.rs
    api/
      mod.rs
      app.rs
      tasks.rs
      aria2.rs
      settings.rs
      debug_logs.rs
      events.rs
    runtime/
      mod.rs
      shutdown.rs
      monitor.rs
    services/
      tasks.rs
      aria2.rs
      settings.rs
    db/
    logs/
    config/
```

也可以短期保留 `src-tauri/` 目录名，但不推荐，因为名字会继续误导架构判断。推荐迁移到 `src-server/` 或 `server/`。

### 6.3 HTTP API

第一阶段 API 建议：

| 方法 | 路径 | 对应现有能力 |
| --- | --- | --- |
| `GET` | `/api/app/info` | `get_app_info` |
| `GET` | `/api/app/ping` | `ping_backend` |
| `GET` | `/api/aria2/status` | `get_aria2_process_status` + RPC status |
| `POST` | `/api/aria2/start` | `start_aria2` |
| `POST` | `/api/aria2/stop` | `stop_aria2` |
| `GET` | `/api/tasks` | `list_download_tasks` |
| `POST` | `/api/tasks` | `create_download_task` |
| `POST` | `/api/tasks/:id/pause` | `pause_download_task` |
| `POST` | `/api/tasks/:id/resume` | `resume_download_task` |
| `POST` | `/api/tasks/:id/redownload` | `redownload_download_task` |
| `DELETE` | `/api/tasks/:id` | `delete_download_task` |
| `GET` | `/api/settings` | `get_app_config` |
| `PUT` | `/api/settings` | `save_app_config` |
| `GET` | `/api/ui-preferences` | `get_ui_preferences` |
| `PUT` | `/api/ui-preferences` | `save_ui_preferences` |
| `GET` | `/api/debug-logs` | `list_debug_logs` |
| `DELETE` | `/api/debug-logs` | `clear_debug_logs` |
| `GET` | `/api/events` | runtime events via SSE |

### 6.4 数据目录策略

当前数据目录来自 Tauri app data dir。FPK 下必须改为明确策略：

- 优先从 fnOS/FPK 注入环境变量读取应用数据目录。
- 若官方模板提供固定路径，按官方模板配置。
- SQLite、Aria2 session、Aria2 log、运行态记录都必须放到 FPK 应用数据目录。
- 下载目录不能默认写死 `~/Downloads`；NAS 环境应默认选择用户授权目录或应用数据目录下的 downloads，并在 UI 中提示用户配置共享目录。

### 6.5 权限和安全

FPK 下必须重新设计：

- 下载目录权限。
- Web UI 访问认证或依赖 fnOS 统一认证。
- API 只监听 `127.0.0.1` 还是通过 fnOS 网关暴露。
- Aria2 RPC secret 仍必须内部随机生成，不对前端暴露。
- 日志继续隐藏私密 URL query。

## 7. 文档整改清单

### 7.1 必须立即更新

| 文档 | 当前问题 | 更新方向 |
| --- | --- | --- |
| `docs/architecture.md` | 把 Tauri 2 定为主应用壳，且 Axum 后期引入 | 改为 FPK-first：Rust server + Vue Web UI + Axum + fnpack |
| `docs/development-plan.md` | 大量阶段状态围绕 Tauri 已完成，误导后续开发 | 新增“架构纠偏阶段”，重置交付目标和验收标准 |
| `README.md` | 若仍描述 Tauri 启动/打包为主 | 改为说明当前处于 FPK-first 整改期 |

### 7.2 必须归档或改名

| 文档 | 处理 |
| --- | --- |
| `docs/ui-stitch-prompts.md` | 保留，UI 设计仍可用于 Web UI |

### 7.3 必须新增

| 文档 | 目的 |
| --- | --- |
| `docs/fnos-fpk-architecture.md` | FPK-first 目标架构 |
| `docs/api-contract.md` | 前后端 HTTP/SSE API 契约 |
| `docs/fpk-packaging.md` | FPK 包结构、fnpack 命令、安装调试流程 |
| `docs/fnos-manual-test-checklist.md` | 飞牛实机安装与基础功能测试清单 |

## 8. 整改阶段计划

### 阶段 0：文档先行，冻结 Tauri 主线

目标：先修正项目的“决策来源”，停止继续沿 Tauri 增加功能，防止方向继续跑偏。

执行规则：

- 本阶段只做文档整改和追踪机制建立，不改代码、不删 Tauri 文件、不启动 server/API 迁移。
- 提交粒度固定为“小任务一提交”。
- 本阶段提交前缀固定为 `docs:`，使用中文 Conventional Commit。
- 完成状态双写：`docs/fnos-fpk-remediation-plan.md` 记录细项，`docs/development-plan.md` 记录阶段摘要。
- 默认在同一提交中更新小任务状态；受 Git 提交哈希自引用限制，`提交记录` 字段允许先写提交主题，并在后续提交中回填前一项短哈希。

执行清单：

| 编号 | 小任务 | 产出 | 验证 | 建议提交 | 状态 | 提交记录 |
| --- | --- | --- | --- | --- | --- | --- |
| P0-1 | 把阶段 0 改成可执行清单 | 为阶段 0 增加编号、跟踪规则和状态表 | 阶段 0 出现覆盖全部后续小任务的执行清单 | `docs: 细化阶段0整改执行清单` | 已完成 | `docs: 细化阶段0整改执行清单`（`efe5a61`） |
| P0-2 | 重写架构目标与总体技术路线 | `docs/architecture.md` 明确 FPK-first、Rust server、Vue Web UI、Axum 主线 | 文档不再把 Tauri 2 写成当前主应用壳 | `docs: 明确FPK-first总体架构主线` | 已完成 | `docs: 明确FPK-first总体架构主线`（`7e2fce6`） |
| P0-3 | 调整架构分层、数据流和运行时表述 | `docs/architecture.md` 切换到 HTTP API / SSE / fnOS 服务生命周期 | 文档不再把托盘、Dock、Tauri sidecar 打包写成长期原则 | `docs: 调整架构分层与数据流到服务化模型` | 已完成 | `docs: 调整架构分层与数据流到服务化模型`（`156283e`） |
| P0-4 | 重置开发计划的目标与现状描述 | `docs/development-plan.md` 顶部改成 FPK-first 目标与 legacy 资产摘要 | 文档顶部不再宣称 Tauri 是当前确定方向 | `docs: 重置开发计划的目标与现状描述` | 已完成 | `docs: 重置开发计划的目标与现状描述`（`aed2264`） |
| P0-5 | 新增架构纠偏阶段并冻结 Tauri 主线 | `docs/development-plan.md` 增加阶段 0、冻结说明和新阶段顺序 | 文档中存在新的“阶段 0：架构纠偏” | `docs: 新增FPK整改阶段并冻结Tauri主线` | 已完成 | `docs: 新增FPK整改阶段并冻结Tauri主线`（`dfbf9d7`） |
| P0-6 | 更新 README 的整改期说明 | `README.md` 切换到 FPK-first 整改叙述 | README 首页不再把 Tauri 桌面应用写成目标形态 | `docs: 更新README说明FPK整改状态` | 已完成 | `docs: 更新README说明FPK整改状态`（`56364d1`） |
| P0-7 | 新增 FPK 文档骨架 | 新建 4 份文档骨架并建立引用入口 | 文档文件存在且标题、命名与整改计划一致 | `docs: 新增FPK架构与交付文档骨架` | 已完成 | `docs: 新增FPK架构与交付文档骨架`（`ef48609`） |
| P0-8 | 做一致性检查并收口阶段状态 | 修正文档冲突并完成阶段 0 状态更新 | 主文档不再把 Tauri 写成当前主线或把 `tauri build` 写成目标交付链路 | `docs: 完成阶段0文档纠偏与状态收口` | 已完成 | `docs: 完成阶段0文档纠偏与状态收口` |

验收：

- 文档不再宣称 Tauri 是目标交付主线。
- 后续任务全部以 FPK/server/Web UI 为验收标准。
- 阶段 0 文档整改已完成，后续可进入 Rust 核心抽离，但仍需保持 legacy 资产可对照迁移。

### 阶段 1：抽出 Rust 业务核心

目标：把可复用业务从 Tauri command 中剥离出来。

执行规则：

- 本阶段保持双轨可运行：`server/` 逐步承载核心逻辑，`src-tauri/` 继续作为 legacy 适配层可编译、可测试。
- 提交粒度固定为“小任务一提交”。
- 代码提交前缀按变更类型使用中文 Conventional Commit，例如 `feat:`、`refactor:`、`test:`、`docs:`。
- 完成状态双写：`docs/fnos-fpk-remediation-plan.md` 记录细项，`docs/development-plan.md` 记录阶段摘要。
- 默认在同一提交中更新小任务状态；受 Git 提交哈希自引用限制，`提交记录` 字段允许先写提交主题，并在后续提交中回填前一项短哈希。

执行清单：

| 编号 | 小任务 | 产出 | 验证 | 建议提交 | 状态 | 提交记录 |
| --- | --- | --- | --- | --- | --- | --- |
| P1-1 | 建立阶段 1 执行清单 | 为阶段 1 增加编号、跟踪规则和状态表 | 阶段 1 出现覆盖后续迁移步骤的执行清单 | `docs: 细化阶段1Rust核心抽离执行清单` | 已完成 | `docs: 细化阶段1Rust核心抽离执行清单`（`247bdfc`） |
| P1-2 | 新建 server 核心库 crate 骨架 | 新建 `server/` 库 crate，并让 `src-tauri` 通过 path dependency 引入 | `cargo test --manifest-path server/Cargo.toml` 与 `cargo test --manifest-path src-tauri/Cargo.toml` 通过 | `feat: 新建server核心库crate骨架` | 已完成 | `feat: 新建server核心库crate骨架`（`3b5b231`） |
| P1-3 | 抽取 config 与 debug_logs | `server/` 承载纯配置与调试日志核心，`src-tauri` 改为包装层或 re-export | `server/src/config`、`server/src/debug_logs` 中不出现 `tauri::` 或 `tauri_plugin` | `refactor: 抽取服务端配置与调试日志核心` | 已完成 | `refactor: 抽取服务端配置与调试日志核心`（`6f8a776`） |
| P1-4 | 抽取 database | `server/` 承载数据库连接、schema 与 repository，`src-tauri` 仅保留 Tauri 路径适配 | `server/src/database` 中不出现 `tauri::Manager`，双轨测试通过 | `refactor: 抽取服务端数据库核心模块` | 已完成 | `refactor: 抽取服务端数据库核心模块`（`3e005fd`） |
| P1-5 | 抽取 tasks 领域核心 | `server/` 承载任务模型、校验、状态变更与 RPC 交互核心 | `server/src/tasks` 中不出现 `tauri::`、`AppHandle`、`Manager` | `refactor: 抽取下载任务领域核心` | 已完成 | `refactor: 抽取下载任务领域核心`（`8f3ad40`） |
| P1-6 | 拆分纯 aria2 核心与 Tauri 进程适配 | `server/` 承载纯 RPC / 端口 / 运行态逻辑，`src-tauri` 保留 sidecar 进程适配 | `server/src/aria2` 中不出现 `AppHandle`、`ShellExt`、`CommandChild` | `refactor: 拆分纯aria2核心与Tauri进程适配` | 已完成 | `refactor: 拆分纯aria2核心与Tauri进程适配`（`d7a7850`） |
| P1-7 | 抽取无 Tauri 的 ServerState | `server/` 新增 `ServerState`，`src-tauri::AppState` 改为 Tauri 适配层 | `server/src/state` 中不出现 `tauri_plugin_shell`、`CommandChild` | `refactor: 抽取无Tauri依赖的ServerState核心` | 已完成 | `refactor: 抽取无Tauri依赖的ServerState核心`（`b1c4089`） |
| P1-8 | 拆分 settings / tasks 服务层并收口 | `server::settings::service` 与 `server::tasks::service` 承接业务编排，`src-tauri` command 变薄 | `cargo check --manifest-path src-tauri/Cargo.toml` 与双轨测试通过 | `refactor: 拆分设置与任务服务层` | 已完成 | `refactor: 拆分设置与任务服务层` |

验收：

- `cargo test` 可在 server crate 独立运行。
- 核心业务不依赖 Tauri。
- SQLite schema 和现有任务测试继续通过。

### 阶段 2：实现 HTTP API 和事件流

目标：用 Axum 替代 Tauri command。

执行规则：

- 本阶段保持双轨可运行：`server/` 逐步成为 HTTP/SSE 主线，`src-tauri/` 继续作为 legacy 适配层可编译、可测试。
- 提交粒度固定为“小任务一提交”。
- 代码提交前缀按变更类型使用中文 Conventional Commit，例如 `feat:`、`fix:`、`refactor:`、`docs:`。
- 完成状态双写：`docs/fnos-fpk-remediation-plan.md` 记录细项，`docs/development-plan.md` 记录阶段摘要。
- 默认在同一提交中更新小任务状态；受 Git 提交哈希自引用限制，`提交记录` 字段允许先写提交主题，并在后续提交中回填前一项短哈希。

执行清单：

| 编号 | 小任务 | 产出 | 验证 | 建议提交 | 状态 | 提交记录 |
| --- | --- | --- | --- | --- | --- | --- |
| P2-1 | 细化阶段 2 执行清单与 API 契约 | 为阶段 2 增加编号、跟踪规则和状态表；补全 `docs/api-contract.md` 初稿 | 文档中出现完整阶段 2 编号、路由清单、错误响应和 SSE 契约 | `docs: 细化阶段2HTTP API执行清单` | 已完成 | `docs: 细化阶段2HTTP API执行清单`（`34b5dbe`） |
| P2-2 | 新增独立 server 启动入口与运行时配置 | 新增 `server/src/main.rs` 与运行时配置加载，建立 `HttpAppState` | `cargo test --manifest-path server/Cargo.toml` 与 `cargo test --manifest-path src-tauri/Cargo.toml` 通过 | `feat: 新增独立server启动入口与运行时配置` | 已完成 | `feat: 新增独立server启动入口与运行时配置`（`30f61ab`） |
| P2-3 | 新增 server 侧 Aria2 进程管理 | 在 `server/` 内承接 Aria2 启停、状态、残留清理和路径解析 | 新增进程管理单测，双轨测试通过 | `feat: 新增server侧Aria2进程管理` | 已完成 | `feat: 新增server侧Aria2进程管理`（`4a5905f`） |
| P2-4 | 新增 Axum 基础接口与统一错误响应 | 建立 `server/src/api`、router 组装、`ApiError` 与 app/aria2 路由 | 新增 handler/router 集成测试 | `feat: 新增Axum基础接口与统一错误响应` | 已完成 | `feat: 新增Axum基础接口与统一错误响应`（`3b96fca`） |
| P2-5 | 新增设置与调试日志 HTTP 接口 | 落地 `settings`、`ui-preferences`、`debug-logs` 路由 | 新增 GET/PUT/DELETE handler 测试，双轨测试通过 | `feat: 新增设置与调试日志HTTP接口` | 已完成 | `feat: 新增设置与调试日志HTTP接口`（`5cb4fc9`） |
| P2-6 | 新增任务 HTTP 接口与自动拉起 Aria2 | 落地 `tasks` 全套路由，并迁移 `ensure_aria2_ready` | 覆盖创建、暂停、继续、删除、退出中拒绝操作 | `feat: 新增任务HTTP接口与自动拉起Aria2` | 已完成 | `feat: 新增任务HTTP接口与自动拉起Aria2`（`9147360`） |
| P2-7 | 新增 SSE 事件流与后台任务同步 | 落地 `/api/events`、`tasks.snapshot` / `runtime.exiting` SSE 事件与 Tokio monitor loop | 新增 SSE 与 monitor 集成测试，双轨测试通过 | `feat: 新增SSE事件流与后台任务同步` | 已完成 | `feat: 新增SSE事件流与后台任务同步`（`4f1be6c`） |
| P2-8 | 完善服务退出清理并收口阶段 2 | 落地优雅关闭、session 保存、Aria2 停止与阶段状态更新 | shutdown 测试与完整回归通过 | `fix: 完善服务退出清理并收口阶段2状态` | 已完成 | `fix: 完善服务退出清理并收口阶段2状态` |

验收：

- 不启动 Tauri 也能通过 HTTP 创建、暂停、继续、删除任务。
- 前端可通过 API 获取任务列表。
- 退出/停止服务时 Aria2 session 保存。

### 阶段 3：前端从 Tauri API 迁移到 HTTP API

目标：把前端主线切到 HTTP + SSE，让 Vue UI 可作为普通 Web UI 运行，同时不新增后端协议、不提前删除 `src-tauri/` Rust legacy 主线。

执行规则：

- 本阶段以 `/api/*` 与 `/api/events` 为唯一前后端通信主线，不新增后端路由。
- 提交粒度固定为“小任务一提交”。
- 代码提交前缀按变更类型使用中文 Conventional Commit，例如 `feat:`、`fix:`、`refactor:`、`docs:`、`chore:`。
- 完成状态双写：`docs/fnos-fpk-remediation-plan.md` 记录细项，`docs/development-plan.md` 记录阶段摘要。
- 默认在同一提交中更新小任务状态；受 Git 提交哈希自引用限制，`提交记录` 字段允许先写提交主题，并在后续提交中回填前一项短哈希。
- Web 版系统集成采用“保留并降级”策略：保存路径保留输入框但取消 Tauri 目录选择器；开机自启/通知开关继续保留为配置项，但不再调用宿主系统插件；不提供 HTTP 版 `quit_app`。

执行清单：

| 编号 | 小任务 | 产出 | 验证 | 建议提交 | 状态 | 提交记录 |
| --- | --- | --- | --- | --- | --- | --- |
| P3-1 | 文档清单与前端迁移矩阵落表 | 为阶段 3 增加执行表、提交规则、迁移范围；在 `docs/api-contract.md` 补充“前端消费方式”说明 | 文档出现完整阶段 3 编号、迁移对象清单、降级策略、验收口径 | `docs: 细化阶段3前端HTTP迁移执行清单`（`7a3cd5b`） | 已完成 | `docs: 细化阶段3前端HTTP迁移执行清单` |
| P3-2 | Web HTTP 基础设施与开发代理 | 新增 `src/services/http.ts`；把 Vite 配置从 Tauri 偏置改为 Web 主线，增加 `/api` 与 SSE 开发代理 | `pnpm run typecheck`、`pnpm run build` | `feat: 新增前端HTTP客户端与Web开发代理` | 已完成 | `feat: 新增前端HTTP客户端与Web开发代理`（`6952f47`） |
| P3-3 | 迁移基础服务到 HTTP | `app`、`aria2`、`settings`、`debug-logs` 服务层改为 HTTP；删除未使用的 `quitApp` 前端出口 | `pnpm run typecheck`、`pnpm run build` | `feat: 迁移基础服务到HTTP接口` | 已完成 | `feat: 迁移基础服务到HTTP接口`（`c162bd9`） |
| P3-4 | 迁移任务服务并降级目录选择交互 | `tasks` 服务改为 HTTP；移除 Tauri 目录选择插件调用；任务新建弹窗改为手动输入保存路径 | `pnpm run typecheck`、`pnpm run build` | `feat: 迁移任务服务并降级目录选择交互` | 已完成 | `feat: 迁移任务服务并降级目录选择交互`（`1e221a6`） |
| P3-5 | 新增前端 SSE 运行时事件服务 | 用 `EventSource` 替换 Tauri `listen`；新增/重命名为 `runtimeEvents` 服务，消费 `tasks.snapshot` 与 `runtime.exiting` | `pnpm run typecheck`、`pnpm run build` | `feat: 新增前端SSE运行时事件订阅` | 已完成 | `feat: 新增前端SSE运行时事件订阅`（`0dabd6a`） |
| P3-6 | 切换任务刷新主路径到 SSE 快照 | `taskStore` 增加快照应用逻辑；`App.vue` / `MainWindow.vue` 改为“首次拉取 + SSE 持续更新”；移除 2 秒轮询主路径 | `pnpm run typecheck`、`pnpm run build`、`rg -n "listen\(|invoke\(" src` 为空 | `refactor: 切换任务刷新主路径到SSE快照` | 已完成 | `refactor: 切换任务刷新主路径到SSE快照`（`917ae12`） |
| P3-7 | 将系统集成功能降级为 Web 安全行为 | `settingsStore` 不再调用 autostart/notification 插件；设置弹窗保留开关但改成“仅保存配置”的 Web 文案 | `pnpm run typecheck`、`pnpm run build` | `fix: 将系统集成功能降级为Web安全行为` | 已完成 | `fix: 将系统集成功能降级为Web安全行为`（`83f98a2`） |
| P3-8 | 清理前端 Tauri 直连依赖并收口阶段 3 | `src/` 内移除 `@tauri-apps/api` 相关导入；删除 `package.json` 中 `@tauri-apps/api` 依赖；完成阶段 3 文档状态更新与最终回归 | `pnpm run typecheck`、`pnpm run build`、`cargo test --manifest-path server/Cargo.toml`、`cargo test --manifest-path src-tauri/Cargo.toml`、`rg -n "@tauri-apps|invoke\(|listen\(" src` 为空 | `chore: 清理前端Tauri直连依赖并收口阶段3` | 已完成 | `chore: 清理前端Tauri直连依赖并收口阶段3` |

补充约定：

- HTTP 客户端默认请求相对路径 `/api/*`，统一处理 JSON、`204 No Content` 与 `{ code, message }` 错误响应。
- Vite 开发代理固定转发 `/api` 与 SSE 到 `http://127.0.0.1:17080`，前端构建态默认同源，不引入新的浏览器端 base URL 配置。
- 运行时事件只消费 `tasks.snapshot` 与 `runtime.exiting`；任务列表刷新切换为“首屏拉取一次 + SSE 快照驱动 + 操作后必要补刷”，不再保留 2 秒固定轮询。
- 阶段 3 收口后，`src/` 内不再出现 `@tauri-apps/api`、`invoke(`、`listen(`。

验收：

- `pnpm run build` 生成纯 Web 静态资源，且构建过程不依赖 Tauri 前端 API。
- 浏览器打开 Web UI 后可完成读取应用信息、任务列表、创建/暂停/继续/重下/删除任务、读取/保存设置、查看/清空调试日志。
- 页面启动后先拉取一次任务列表，后续通过 SSE 接收 `tasks.snapshot`。
- 收到 `runtime.exiting` 后，前端会进入退出态并禁用任务操作。
- `rg -n "@tauri-apps|invoke\(|listen\(" src` 结果为空。
- `src-tauri` Rust 仍可测试通过，未提前删除 legacy 后端入口。

### 阶段 4：建立 FPK 打包链路

目标：生成可在飞牛应用中心安装的 `.fpk`。

执行规则：

- 本阶段保持 server / Web UI 主线可构建，legacy `src-tauri/` 仅继续承担回归参照，不新增 Tauri-only 打包能力。
- 提交粒度固定为“小任务一提交”。
- 代码提交前缀按变更类型使用中文 Conventional Commit，例如 `feat:`、`fix:`、`chore:`、`docs:`。
- 完成状态双写：`docs/fnos-fpk-remediation-plan.md` 记录细项，`docs/development-plan.md` 记录阶段摘要。
- 默认在同一提交中更新小任务状态；受 Git 提交哈希自引用限制，`提交记录` 字段允许先写提交主题，并在后续提交中回填前一项短哈希。

执行清单：

| 编号 | 小任务 | 产出 | 验证 | 建议提交 | 状态 | 提交记录 |
| --- | --- | --- | --- | --- | --- | --- |
| P4-1.1 | 生成基准 FPK 目录结构 | 新建 `packaging/fnos/` 基础目录骨架，并补充阶段 4 跟踪表 | `packaging/fnos/` 下存在 `app/`、`cmd/`、`config/`、`ui/`、`wizard/` 结构 | `feat: 新建FPK打包目录骨架` | 已完成 | `feat: 新建FPK打包目录骨架`（`0eaa0d3`） |
| P4-1.2 | 补齐基础图标与资源占位 | 放入 fnOS 图标资源，占位 manifest 引用所需文件名 | 图标文件存在，且 FPK 主线不再依赖 Tauri 图标集合 | `feat: 补齐FPK基础图标资源` | 已完成 | `feat: 补齐FPK基础图标资源`（`c433f9f`） |
| P4-2.1 | 补齐 Linux x86_64 server 构建产物 | 固定 server release 产物命名与打包输入路径 | `pnpm run build:server:linux:x64` 通过，并生成标准产物路径 | `feat: 补齐Linux x86_64服务端构建产物` | 已完成 | `feat: 补齐Linux x86_64服务端构建产物`（`7ec9d75`） |
| P4-2.2 | 打通 Vue `dist/` 输出 | 固定 Web UI 构建与拷贝目标目录 | `pnpm run build:web:fpk` 通过，且 `packaging/fnos/ui/dist/index.html` 存在 | `feat: 打通Web UI构建产物输出` | 已完成 | `feat: 打通Web UI构建产物输出`（`487b041`） |
| P4-2.3 | 整理 Aria2 Next sidecar 放置规则 | 固定 Linux sidecar 打包路径与命名 | `pnpm run stage:aria2:x64` 成功，且脚本支持 `aarch64-unknown-linux-gnu` dry-run 校验 | `feat: 整理Aria2 Next sidecar打包规则` | 已完成 | `feat: 整理Aria2 Next sidecar打包规则`（`56b3bbc`） |
| P4-3.1 | 编写 FPK 启动脚本 | 新增 `cmd/start`，注入数据目录、端口、日志等运行参数 | 手动执行 `packaging/fnos/cmd/start` 可启动服务并产生日志 | `feat: 新增FPK启动脚本` | 已完成 | `feat: 新增FPK启动脚本`（`cbdb085`） |
| P4-3.2 | 编写 FPK 停止脚本 | 新增 `cmd/stop`，停止服务并清理运行态 | 手动执行 `packaging/fnos/cmd/stop` 后进程退出 | `fix: 新增FPK停止脚本与清理逻辑` | 已完成 | `fix: 新增FPK停止脚本与清理逻辑` |
| P4-3.3 | 编写 FPK 状态脚本 | 新增 `cmd/status`，输出服务状态和退出码 | 服务运行/停止时状态输出与退出码正确 | `feat: 新增FPK状态查询脚本` | 未开始 |  |
| P4-3.4 | 补齐 manifest 与权限配置 | 配置应用入口、端口、图标、权限 | manifest 字段可被 `fnpack` 接受，且与脚本入口一致 | `feat: 配置FPK manifest与权限` | 未开始 |  |
| P4-4.1 | 新增 FPK 一键打包脚本 | 串联前端构建、server 构建、sidecar 拷贝和 `fnpack build` | 单脚本可完成全流程，失败时有明确报错 | `feat: 新增FPK一键打包脚本` | 未开始 |  |
| P4-4.2 | 统一脚本与 CI 构建入口 | 本地与 CI 复用同一套 FPK 构建入口 | 本地验证入口可运行，CI 命令不冲突 | `chore: 统一FPK构建入口` | 未开始 |  |
| P4-5.1 | 执行完整打包验证 | 生成并检查 `.fpk` 产物内容 | 本地运行 `fnpack build` 成功生成 `.fpk` | `fix: 修复FPK打包链路校验问题` | 未开始 |  |
| P4-5.2 | 更新阶段 4 文档状态 | 同步整改计划与开发计划的阶段 4 完成状态 | 文档状态与代码状态一致，可进入阶段 5 | `docs: 收口阶段4FPK打包链路状态` | 未开始 |  |

验收：

- 本地或飞牛上执行 `fnpack build` 生成 `.fpk`。
- `.fpk` 可被飞牛应用中心识别安装。

### 阶段 5：飞牛实机安装和基础功能验证

目标：确认最小可用。

任务：

1. 在飞牛应用中心手动安装 `.fpk`。
2. 启动应用。
3. 打开 Web UI。
4. 新建 HTTP/HTTPS 下载任务。
5. 验证任务列表、进度、速度、大小。
6. 验证暂停、继续、删除。
7. 验证设置保存。
8. 验证退出/停止应用后 Aria2 停止且 session 保存。
9. 验证重启后未完成任务默认暂停，可手动继续并断点续传。
10. 验证诊断日志可查看和复制。

验收：

- 基础下载闭环在飞牛上可用。
- FPK 安装、启动、停止、卸载无明显残留。

## 9. 删除顺序

为避免一次删除导致不可运行，删除必须分阶段执行。

### 第一批：立即可删

这些是垃圾或脚手架残留：

- `src-tauri/src/aria2/mod.rs.tmp`
- `src/assets/vue.svg`
- `public/tauri.svg`
- `public/vite.svg`
- 未跟踪的 `.DS_Store` 文件

### 第二批：HTTP/Web UI 迁移完成后删除

- `src/services/*` 中所有 Tauri invoke/listen 实现。
- `src/features/settings/services/systemIntegrationService.ts` 中 Tauri 系统集成。
- `package.json` 中 `@tauri-apps/api` 依赖。

### 第三批：server + FPK 可运行后删除或归档

- `src-tauri/tauri.conf.json`
- `src-tauri/build.rs`
- `src-tauri/capabilities/`
- `scripts/tauri-dev.mjs`
- `package.json` 中 Tauri scripts。
- `src-tauri/icons/` 中非 FPK 图标。
- `src-tauri/binaries/aria2-next-aarch64-apple-darwin`
- `@tauri-apps/cli` 和所有 Tauri plugin 依赖。

### 第四批：确认无回退需求后删除

- Tauri `lib.rs` 中窗口、托盘、菜单、Dock、插件相关逻辑。
- `src-tauri/src/commands/`。
- `src-tauri/src/runtime/mod.rs` 中 Tauri 通知逻辑。

## 10. 风险和注意事项

### 10.1 下载目录权限风险

桌面默认 `~/Downloads` 不适合 NAS。FPK 必须按 fnOS 权限和用户授权目录设计。

### 10.2 端口和网关风险

当前 Aria2 RPC 端口管理是内部使用。FPK 还需要管理后端 HTTP 服务端口，并与 fnOS 应用入口/网关配置一致。

### 10.3 安全风险

HTTP API 不能直接裸露到局域网，除非接入 fnOS 认证或后端自带鉴权。

### 10.4 架构迁移风险

如果直接删除 Tauri 而未完成 server/API，项目会短期不可运行。应先抽核心、再替换入口。

### 10.5 文档可信度风险

当前 `docs/architecture.md` 和 `docs/development-plan.md` 已不可信，必须在阶段 0 修正，否则后续开发会继续被错误主线牵引。

## 11. 优先级总表

| 优先级 | 任务 | 原因 |
| --- | --- | --- |
| P0-0 | 更新架构、开发计划和 README | 文档是后续实现的决策来源，必须先纠偏 |
| P0-1 | 标注旧 runtime lifecycle 文档适用边界 | 避免把 Tauri 窗口/托盘语义继续带入 FPK |
| P0 | 抽出 Rust 核心，去 Tauri 依赖 | FPK 的必要前置 |
| P0 | 建立 HTTP API | 替代 Tauri command |
| P0 | 前端替换 Tauri invoke | Web UI 必需 |
| P1 | 建立 FPK 包结构和 fnpack 构建 | 最终交付必需 |
| P1 | 飞牛实机安装验证 | 验证目标是否达成 |
| P2 | 删除 Tauri 残留 | 等替代链路稳定后执行 |
| P2 | 完善认证、网关、权限 | 上架或长期使用前必须补齐 |

## 12. 立即下一步

建议按以下顺序执行：

1. 更新 `docs/architecture.md` 为 FPK-first 架构。
2. 更新 `docs/development-plan.md`，把当前 Tauri 工作标记为 legacy/可迁移资产，不再算最终交付完成。
3. 更新 `README.md`，明确当前项目处于 FPK-first 整改期，暂不能直接打包为可安装 FPK。
4. 新建 server crate，引入 Axum。
5. 先迁移 `get_app_info`、`ping_backend`、`list_debug_logs` 这类低风险 API，跑通前后端 HTTP 通信。
6. 再迁移 tasks/aria2/settings。
7. 最后建立 `packaging/fnos/` 和 `fnpack build`。

本计划执行完成前，不应继续新增 Tauri-only 功能。
