# 飞牛版 Motrix 架构文档

> 本文档负责约束项目的整体技术架构、前后端职责边界、UI 组件策略和目录组织。具体阶段任务、完成状态和优先级放在 `docs/development-plan.md`。

## 1. 架构目标

飞牛版 Motrix 的目标不是简单复刻桌面版 Motrix，而是在飞牛 OS 场景下提供一个可安装、可运行、可长期维护的 GUI 下载工具。

架构必须满足以下目标：

- **可运行**：在飞牛 OS 上以 GUI 应用形态运行，并最终以 FPK 形式交付。
- **可维护**：前端、后端、下载引擎、持久化、日志各自边界清晰，避免单文件堆叠。
- **可恢复**：任务、配置、历史状态最终必须可持久化，支持应用重启后恢复。
- **可排障**：生产环境不能依赖开发终端，必须有应用内日志和诊断入口。
- **可扩展**：后续能自然扩展 BT、磁力、批量任务、远程管理和自动化能力。

## 2. 总体技术路线

当前项目采用以下技术路线：

- 应用壳：**Tauri 2**
- 后端核心：**Rust**
- 下载引擎：**Aria2 Next sidecar**
- 前端框架：**Vue 3 + TypeScript + Vite**
- UI 组件库：**Naive UI**
- 前端状态管理：**Pinia**
- 本地持久化：**SQLite**
- Rust 异步运行时：**Tokio**
- Rust 序列化：**Serde**
- Rust 数据库访问：**SQLx**
- 日志与诊断：**tracing + 应用内调试日志队列**
- 远程 API：**Axum 后期引入**，第一版不加入

## 3. 分层职责

### 3.1 Rust 核心层

Rust 负责核心业务能力，不负责 UI 展示。

职责：

- 业务模型定义
- 下载任务生命周期管理
- 配置读取与校验
- 下载目录、文件路径、安全边界处理
- 启动、停止、监控 Aria2 Next sidecar
- 与 Aria2 JSON-RPC 通信
- SQLite 持久化
- 应用内日志写入
- 暴露 Tauri command 给前端调用

不职责：

- 页面布局
- 组件交互
- 表格渲染
- 前端临时状态管理

### 3.2 Aria2 Next 下载引擎层

Aria2 Next 只负责真实下载能力。

职责：

- HTTP / HTTPS / BT / 磁力等底层下载
- 分片、并发、断点续传、限速等下载能力
- 通过 JSON-RPC 接受任务控制
- 返回任务状态、进度、速度、错误码和文件信息

不职责：

- 任务列表 UI
- 应用配置存储
- 历史记录存储
- 飞牛系统集成

### 3.3 Tauri 应用壳层

Tauri 负责桌面/飞牛 GUI 应用壳和系统能力接入。

职责：

- 创建应用窗口
- 前端与 Rust command 通信
- 文件/目录选择等系统能力
- 打包 sidecar
- 后续托盘、菜单、后台驻留、自启动等能力
- 最终参与 FPK 交付链路

不职责：

- 下载业务本身
- 前端页面状态
- 任务表格交互

### 3.4 Vue 前端层

Vue 负责 UI 展示和用户交互，不直接承载后端业务规则。

职责：

- 主窗口布局
- 任务列表、任务详情、新建任务、设置、诊断日志等页面
- 用户输入校验中的轻量即时反馈
- 调用 service / store 完成交互
- 表格、弹窗、Toast、菜单等交互组件

不职责：

- 真实下载逻辑
- 文件系统权限判断
- 持久化数据库读写
- 直接拼装复杂 Aria2 RPC 请求

### 3.5 Pinia 状态层

Pinia 负责前端全局和跨组件状态。

职责：

- 任务列表状态
- 当前筛选条件
- 当前选中任务
- 任务轮询状态
- 配置缓存
- UI 偏好，例如表格列宽、当前分类、侧栏展开状态

不职责：

- SQLite 持久化
- Aria2 RPC 直接调用
- 复杂后端业务判断

### 3.6 SQLite 持久化层

SQLite 是本地长期状态来源。

职责：

- 下载任务记录
- 配置项
- 历史任务
- 错误记录
- UI 偏好中需要长期保存的部分，例如任务表列宽

不职责：

- 当前页面临时状态
- 实时渲染
- Aria2 实时下载执行

## 4. 前端架构规范

### 4.1 目录结构

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
        TaskTable.vue
        TaskCreateDialog.vue
        TaskStatusBadge.vue
        TaskProgressCell.vue
        TaskActions.vue
      stores/
        taskStore.ts
      composables/
        useTaskPolling.ts
      services/
        taskService.ts
      types.ts
    diagnostics/
      components/
        DiagnosticsDialog.vue
        DebugLogDialog.vue
      stores/
        debugLogStore.ts
      services/
        debugLogService.ts
  services/
    tauri.ts
    aria2.ts
  types/
    app.ts
    aria2.ts
```

说明：

- `views/` 只放页面入口，负责组合布局和功能模块。
- `layouts/` 放通用窗口结构，例如侧栏、顶部栏、整体 shell。
- `features/` 按业务领域拆分，任务、日志、设置等都应进入各自 feature。
- `components/` 放 feature 内部组件，避免所有组件都堆在全局目录。
- `stores/` 放 Pinia store。
- `composables/` 放可复用交互逻辑，例如轮询、拖拽、快捷键等。
- `services/` 放前端到 Tauri command 的调用封装。
- `types/` 放类型定义，避免在视图组件里散落接口定义。

### 4.2 `MainWindow.vue` 边界

`MainWindow.vue` 只允许承担页面编排职责。

允许：

- 引入布局组件
- 引入任务模块组件
- 引入诊断模块组件
- 处理极少量页面级开关状态

不允许：

- 直接实现完整任务表
- 直接实现复杂弹窗表单
- 直接实现 Toast 队列
- 直接实现任务轮询
- 直接写大量 CSS 模拟组件库能力
- 直接调用 Tauri command 绕过 feature service / store

任务列表、创建弹窗、Toast、轮询、诊断弹窗和复杂交互应拆入对应 layout / feature；后续不得重新堆回 `MainWindow.vue`。

## 5. UI 组件策略

### 5.1 正式采用 Naive UI

项目正式采用 **Naive UI** 作为 Vue UI 组件库。

原因：

- 与 Vue 3 / TypeScript 适配成熟。
- 提供 DataTable、Dialog、Message、Notification、Form、Input、Button、Tabs 等基础能力。
- 可以减少自研控件带来的交互和可维护性成本。
- 适合先快速做出稳定工具，再逐步定制视觉风格。

### 5.2 组件库使用原则

优先使用 Naive UI：

- 表格：`NDataTable`
- 弹窗：`NModal` / `NDialog`
- 表单：`NForm` / `NInput` / `NSelect`
- 按钮：`NButton`
- Tabs：`NTabs`
- Toast / 提示：`NMessage` / `NNotification`
- 进度：`NProgress`
- 空状态：`NEmpty`
- 滚动容器：优先使用组件内建能力或简单 CSS，不重复造复杂交互

允许自定义 CSS：

- 应用整体暗色主题
- 飞牛/Motrix 风格的颜色、间距、圆角
- 侧栏和窗口 shell 的产品化布局
- 少量 Naive UI 组件无法覆盖的展示细节

不建议自研：

- 表格列宽拖动
- 表头固定
- 虚拟列表
- 弹窗焦点管理
- Toast 队列管理
- 表单校验框架

### 5.3 组件优先级与自研原则

UI 开发按以下优先级决策：

1. **优先使用 Naive UI 官方组件**：只做主题、样式和组合层面的适配，不重复实现组件库已经稳定提供的能力。
2. **其次使用维护良好的现成轮子**：仅在 Naive UI 无法满足需求时考虑；引入前必须评估维护状态、许可证、依赖体积、与 Vue 3 / TypeScript / Tauri 的兼容性，以及是否会增加长期维护成本。
3. **最后才自研组件**：只有在 Naive UI 和合适第三方组件都无法满足需求，或引入成本明显高于自研成本时，才允许自研。

自研组件必须遵守：

- 页面、业务逻辑、状态管理、服务调用和展示组件分离。
- 单个组件只承担清晰职责，不把页面布局、数据请求、复杂交互和样式全部堆在一个文件里。
- 复杂交互应拆成 composable，例如轮询、拖拽、快捷键、滚动同步等。
- 可复用展示单元应拆成小组件，例如状态徽标、进度单元格、操作按钮组等。
- 组件 API 要清晰，通过 props / emits / slots 交互，避免隐式依赖全局状态。
- 仅当状态跨组件共享或需要长期维护时进入 Pinia；纯展示状态留在局部组件。
- 自研前要先说明为什么 Naive UI 或现成轮子不适用。

## 6. 任务表架构

### 6.1 任务表必须使用 DataTable

任务列表是项目核心交互，不应继续使用手写 CSS Grid 模拟表格。

目标实现：

- 使用 Naive UI `NDataTable`
- 每一列显式定义 `key`、`title`、`width`、`minWidth`
- 需要用户调整的列设置 `resizable: true`
- 使用 `scroll-x` 支持横向滚动
- 使用 DataTable 内建滚动区域承载纵向滚动
- 操作列保留给暂停、继续、删除、打开目录等动作

### 6.2 列宽拖动和横向滚动的边界

任务表必须区分两种交互：

- **拖动表头列边界**：调整列宽。
- **滚轮/触控板横向滑动或滚动条**：横向滚动表格。

不应把整个任务表容器绑定为“鼠标左右拖动 = 横向滚动”。这种做法会和列宽拖动、文本选择、按钮点击冲突。

### 6.3 推荐列定义

首版任务表列：

| 列 | key | 建议宽度 | 可拖动 | 说明 |
| --- | --- | ---: | --- | --- |
| 任务名称 | name | 360 | 是 | 文件名、URL、失败原因 |
| 状态 | status | 110 | 是 | 状态徽标 |
| 进度 | progress | 180 | 是 | 进度条和百分比 |
| 已下载 / 总大小 | size | 180 | 是 | 格式化大小 |
| 速度 | speed | 130 | 是 | 实时速度 |
| 剩余时间 | eta | 120 | 是 | ETA |
| 保存路径 | saveDir | 320 | 是 | 路径省略展示，完整路径 title |
| 操作 | actions | 140 | 否 | 暂停、继续、删除、更多 |

后续可增加：

- 创建时间
- 完成时间
- 分类
- 来源协议
- 错误码
- GID

### 6.4 列宽持久化

列宽属于 UI 偏好。

阶段性策略：

1. 前端本地 store 中维护列宽。
2. SQLite 配置表完成后，持久化列宽。
3. 用户重启应用后恢复列宽。

## 7. 前后端数据流

标准数据流：

```text
Vue Component
  -> Pinia Store
  -> Feature Service
  -> Tauri invoke wrapper
  -> Rust Tauri Command
  -> Rust Service / Repository
  -> Aria2 JSON-RPC / SQLite
```

禁止：

- Vue 组件直接散落调用 `invoke`。
- Vue 组件直接拼 Aria2 RPC 参数。
- Rust command 内直接写大量业务逻辑而不拆 service。
- UI 状态和后端状态混在同一个对象里无法区分。

允许：

- 简单只读状态在组件内局部维护。
- 页面级显示开关在页面入口维护。
- 后端 command 做参数校验入口，但复杂逻辑应进入模块函数。

## 8. Rust 后端架构规范

目标 Rust 目录结构：

```text
src-tauri/src/
  main.rs
  lib.rs
  app/
    state.rs
  commands/
    tasks.rs
    aria2.rs
    logs.rs
    config.rs
  tasks/
    mod.rs
    model.rs
    service.rs
  aria2/
    mod.rs
    process.rs
    rpc.rs
    model.rs
  config/
    mod.rs
    aria2.rs
    app_config.rs
  db/
    mod.rs
    migrations.rs
    repositories/
      tasks.rs
      config.rs
  logs/
    mod.rs
    model.rs
    ring_buffer.rs
```

说明：

- `commands/` 只做 Tauri command 入口和参数/错误转换。
- `tasks/` 负责任务业务逻辑。
- `aria2/` 负责进程管理和 JSON-RPC。
- `config/` 负责配置读取与默认值。
- `db/` 负责 SQLite 初始化、迁移和 repository。
- `logs/` 负责内存日志队列和后续落盘日志。

## 9. 错误和日志规范

### 9.1 面向用户的错误

用户可见错误必须：

- 使用中文描述。
- 告诉用户当前失败在哪一步。
- 尽量给出下一步动作，例如检查路径权限、稍后重试、查看调试日志。
- 不暴露 RPC secret。
- 不完整暴露带私密 query 的下载链接。

### 9.2 面向排障的日志

日志必须覆盖关键链路：

- 应用启动
- sidecar 路径与启动来源
- sidecar 启动参数摘要
- RPC ready / timeout / failed
- CA 证书探测
- 下载目录解析与创建
- `aria2.addUri`
- `aria2.tellStatus`
- 任务状态变化
- SQLite 迁移和读写错误

日志字段至少包括：

- 时间
- 级别：info / warn / error
- 来源模块
- 消息
- 可选上下文

## 10. 长期演进关注点

本节只记录持续性架构关注点；一次性阶段任务和完成状态应放入 `docs/development-plan.md`。

- **任务控制**：暂停、恢复、删除等能力必须通过 Rust command -> Rust service -> Aria2 RPC 的链路实现，前端只通过任务 store / service 调用。
- **SQLite 持久化**：任务、配置、历史、错误记录和需要长期保存的 UI 偏好都应规划 SQLite 表或配置项，不把长期状态只留在内存中。
- **诊断日志**：生产环境排障必须依赖应用内日志入口，不依赖开发终端；日志不得泄漏 RPC secret 或完整私密下载链接 query。
- **前端边界**：新增任务相关代码进入 `features/tasks`，新增诊断/日志相关代码进入 `features/diagnostics`，页面入口只做组合。

## 11. 开发约束

后续开发必须遵守：

- 不在 `MainWindow.vue` 继续堆新功能。
- 新任务相关 UI 必须进入 `features/tasks`。
- 新诊断相关 UI 必须进入 `features/diagnostics`。
- 前端调用后端必须经过 service / store 封装。
- 任务表必须使用 Naive UI `NDataTable`，不得回退或继续扩展自研 Grid 方案。
- 复杂 UI 交互优先使用 Naive UI 能力。
- 如果 Naive UI 无法满足需求，应先评估现成轮子；确需自研时必须按页面、逻辑、状态和组件拆分。
- 后端新增 command 时必须明确属于哪个模块。
- 新增长期状态时必须考虑 SQLite 持久化路径。
- 生产环境排障能力必须优先于复杂功能堆叠。

## 12. 与开发计划的关系

本文档回答“项目应该如何组织和演进”。

`docs/development-plan.md` 回答“现在做什么、做到什么程度、如何验收”。

当两者冲突时：

1. 以架构文档定义的职责边界为准。
2. 开发计划可以调整阶段顺序，但不应绕过架构边界继续堆叠实现。
3. 如架构文档不适合当前实际，应先更新架构文档，再继续实现。
