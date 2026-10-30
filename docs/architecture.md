# 飞牛版 Motrix 架构说明

本文只讲长期结构和关键底线。阶段事项见 [开发计划](future-development-plan.md)，开发规则见 [AGENTS.md](../AGENTS.md)，操作步骤见 [维护手册](maintenance.md)。

## 1. 项目与技术栈

Motrix 是交付为 `.fpk` 的 fnOS 下载管理应用，维护主线为 `server/`、`src/` 和 `packaging/fnos/`。

| 层级 | 选型与职责 |
| --- | --- |
| FPK | 元数据、权限、入口、生命周期脚本和产物组装 |
| Rust server | Tokio、Axum、Serde、SQLx、SQLite、tracing；业务、路径安全、引擎协调、HTTP/SSE |
| Aria2 Next | 独立 sidecar；HTTP/HTTPS、BT、磁力、续传、限速和状态上报 |
| Web UI | Vue 3、TypeScript、Vite、Naive UI；页面交互与展示 |
| Pinia | 前端任务快照、筛选、选中项、事件和配置缓存，不代替后端持久化 |

## 2. 运行模型

```text
fnOS FPK 生命周期脚本
  └─ Rust server（共享业务状态、SQLite 和退出信号）
       ├─ 管理入口：Web UI / HTTP API / SSE，IPv4 + IPv6
       ├─ 回环 RPC：仅 /jsonrpc，供 NAS 本机反向代理
       ├─ 局域网 RPC：仅 /jsonrpc，独立 Token 和真实来源限制
       └─ Aria2 Next sidecar：由生命周期协调器统一管理
```

三类入口不得混用凭据；默认地址、来源策略、配额和失败响应统一见 [接口运行约定](api-contract.md#1-运行时约定)。局域网 RPC 端口占用允许明确降级，其他地址绑定失败须整体启动失败，不留下半可用服务。

正式包使用端口入口；独立 Probe 不属于正式拓扑。FPK 身份、双架构构建和平台依据见 [维护手册](maintenance.md#打包与发布)。

## 3. 目录与职责

| 目录 | 职责 |
| --- | --- |
| `src/views/`、`layouts/` | 页面入口和布局；`MainWindow.vue` 只编排，不直接做接口调用、轮询、任务表或复杂弹窗 |
| `src/views/composables/` | 主窗口弹窗、启动与退出刷新编排 |
| `src/features/*/` | 按领域组织组件、store、service、composable 和类型；跨页面任务操作放 tasks 领域 |
| `src/services/` | 统一 HTTP client 与 SSE 订阅封装 |
| `src/app/providers/`、`src/styles/` | 全局主题、基础样式和公共 UI 约定 |
| `server/src/api/` | HTTP handler 与请求响应转换，不堆业务逻辑 |
| `server/src/auth/` | 密码、JWT、限速和认证中间件，独立于下载设置 `AppConfig` |
| `server/src/tasks/` | 任务业务；`service.rs` 注入依赖、共享守卫、创建对账和回滚辅助，`service/` 子模块承载各流程 |
| `server/src/aria2/`、`runtime/` | 引擎 RPC、进程和运行态协调 |
| `server/src/settings/`、`storage/`、`database/`、`debug_logs/` | 设置、授权与文件、持久化、日志领域 |
| `packaging/fnos/` | FPK manifest、权限、入口、向导和生命周期脚本，不承载下载业务 |

后端新增能力按 `api -> 领域 service -> domain -> persistence` 分层，不另建脱离领域的通用业务层。

## 4. 前端与数据流

桌面浏览器、手机浏览器和飞牛 App WebView 共用 Vue、Pinia、service、HTTP API 和 SSE。布局及展示可按屏幕拆分，业务状态和操作必须复用。

```text
Vue Component -> Pinia -> Feature Service -> HTTP client（JWT Bearer）
  -> 管理入口 Axum Route -> Rust Service / Repository -> Aria2 / SQLite
Rust Runtime Event -> SSE -> runtime event service -> Pinia -> Component
```

- 不在组件散落后端请求，不在前端拼装复杂 Aria2 RPC，不混用页面临时状态与持久状态。
- UI 优先使用 Naive UI；主题、样式文件及 UnoCSS 使用规则见 [开发指令](../AGENTS.md#实现与文档)，产品交互和视觉要求见 [UI 规范](design/DESIGN.md)。
- HTTP 与 SSE 使用同源请求和 `Authorization: Bearer <JWT>`；上游网关 Cookie 只用于网关访问，不作为 Motrix 授权依据。
- fnOS SDK 只在宿主内由用户操作触发；独立浏览器显示降级提示。授权事实始终来自服务端官方 API 快照，SDK 返回值和语义化路径不能绕过真实路径校验。

## 5. 持久化与任务安全

- SQLite 保存任务身份、目录授权、文件选择、代理意图、回收站及操作事实；session 只作恢复输入，不能反向覆盖应用事实。
- 引擎负责 HTTP 恢复记录、BT fast-resume、完整 metadata 和 session；应用不修改或复制运行中的引擎数据库行。BT partfile 等实际数据按下载内容管理。
- SQLite、私有 `state-dir`、session、种子备份、日志和运行态文件都放 FPK 应用数据目录，不依赖临时目录或用户下载目录。
- 内置与外部路径只运行已验证的 Aria2 Next 2.8.6，统一使用 NativeV2；旧版、未知或不可用引擎进入受限管理，保留旧状态和用户文件。
- 2.x 不接管 1.x 任务或半成品；密码、设置、授权和 RPC Token 沿用。升级前须由用户确认兼容性与备份说明，确认不改变数据保留规则。兼容边界见 [应用信息契约](api-contract.md#42-应用信息)，升级操作见 [备份与回滚](maintenance.md#备份与回滚)。
- 保留已发布 schema 迁移、旧代存储值与历史数据，不再维护旧代引擎的执行算法。
- 单条、批量 URL、种子和磁链复用校验、选项映射与持久化；批量允许部分成功，不回滚已成功任务。
- 磁链在私有目录解析完整 metadata，保持原 GID，确认文件后才向授权下载目录写内容；种子与确认后的磁链使用任务专属目录。
- 暂停和回收站恢复必须保留原身份、文件选择与有效恢复信息，重启先暂停对账；状态缺失时保留文件并报错，不新建 GID 接管半成品。
- 普通删除保留文件；永久删除才按明确选择清理下载内容。写入、暂存、删除前重新校验当前授权和可证明的文件归属，禁止删除授权根目录。
- 暂停未确认不得暂存或删除文件。永久删除使用原子暂存和有界后台清理，成功前保留记录；重新下载先可靠建立新暂停任务，失败须恢复旧任务和文件。
- 原始种子、metadata 和恢复信息关联应用任务；回收站未清空时不自动过期，共享 metadata 无其他引用后才清理。
- HTTP 代理意图按任务保存，BT 使用应用总开关；代理未配置、应用失败或结果未知时保持暂停，不静默直连。详细事务、兼容覆盖及连接参数见 [任务](api-contract.md#44-任务)和 [设置契约](api-contract.md#45-设置)。
- BT 代理须在任何网络活动前核对，覆盖 peer、tracker、web-seed、DNS 和发现渠道。
- 普通 HTTP/HTTPS 按原文件下载，不自动媒体解析；敏感 Authorization/Cookie 不跨来源转发，自定义 HTTP Header 不传给 BT tracker/web-seed。

## 6. 认证与边界

- Web 管理始终使用自身密码和 JWT，不依赖端口入口中的 fnOS 身份 Header，也不提供关闭密码保护的配置。
- 管理密码在安装向导初始化；网页只提供登录、恢复提示及脱敏诊断。本机停服后才能重置密码。哈希、JWT 和限速细节见 [鉴权契约](api-contract.md#41-web-管理鉴权)。
- 密码不得进入命令参数、日志或应用持久化明文文件；安装向导字段的平台保留行为须另行实机核验。
- Web 管理密码、公网 RPC Token、局域网 RPC Token 相互独立；Aria2 内部 secret 只能由服务端生成和持有。
- 公网反代只指向回环 RPC 入口，不用来源 IP、Host 或可伪造 Header 区分管理面与公网 RPC 面；局域网来源只按真实 TCP 对端判断。
- 日志、SSE、操作记录和诊断不暴露密码、JWT、Token、私密 URL 或代理凭据；原生恢复状态不进入日志清理和诊断导出。

## 7. 生命周期

- fnOS `start` / `stop` / `status` 是应用生命周期入口；关闭或刷新前端页面不等于应用退出。
- 启停及状态核验 PID、启动时间和可执行文件；孤儿清理还须核验 UID 与归属，禁止按宽泛进程名或端口杀进程。
- Aria2 端口、secret、句柄、RPC ready、运行态和启停都由同一协调器管理；任务、外部创建、恢复和监控不能绕过它。
- server 启动先准备目录与 SQLite，引擎按需启动；读取接口不唤醒引擎，显式手动启动允许无任务启动。
- 下载、metadata、做种、未排空 I/O、引擎操作和排队请求才算活动；只有暂停、完成、错误、待确认或回收站 GID 时，协调器防抖并二次确认空闲后保存状态、session、停止进程，再清运行态。
- 手动停止遇活动任务或在途操作返回冲突，不隐式暂停；空闲停止不复用完整应用退出的暂停语义。停止期间新请求须取消停止、等待重启或得到明确可重试错误。
- server 退出立即标记不可就绪并停止接新请求；HTTP 排空、任务持久化、RPC、session / 原生状态保存和 sidecar 停止共享有界总预算，超时记录未完成阶段。
- 引擎退出超时保留句柄和结果，不重复发信号或等待已关闭 RPC；在生命周期锁内核对迟到结果，正常退出才清状态并允许重启，未退出可重试，异常退出明确报错。
- fnOS 停止脚本有界升级信号；只有确认 server 和所属 sidecar 都退出才允许删包或应用数据，否则保留状态和文件并失败关闭。
- 未知 RPC 结果、GID、暂存文件或 metadata 问题保留用户文件并转诊断处理；操作方法见 [维护手册](maintenance.md#故障排查)。
