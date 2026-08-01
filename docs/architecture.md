# 飞牛版 Motrix 架构文档

> 本文档只约束长期架构、职责边界、目录组织和运行模型。阶段任务、状态和优先级见 `docs/development-plan.md`；接口细节见 `docs/api-contract.md`；开发命令见 `docs/development-scripts.md`；FPK 打包与实机检查见 `docs/fpk-packaging.md`；当前 UI 产品需求与视觉规则见 `docs/design/ui-product-requirements.md` 和 `docs/design/DESIGN.md`。

## 1. 架构边界

本项目交付 **fnOS FPK 下载管理应用**。

固定边界：

- 交付形态：`.fpk`。
- 运行模型：fnOS 服务启动 Rust server，server 托管 Web UI 并管理 Aria2 Next sidecar。
- 前后端通信：同一 Rust server 共享业务状态并启动三个 TCP 监听器；管理监听器承载 Web UI、HTTP API 与 SSE，回环 RPC 监听器和局域网 RPC 监听器分别承载使用独立 token 的 JSON-RPC 兼容入口。
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
  │   ├─ 管理监听器（Web UI、HTTP API、SSE）
  │   ├─ 回环 RPC 监听器（仅 /jsonrpc）
  │   ├─ 局域网 RPC 监听器（仅 /jsonrpc）
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
- Web UI、HTTP API 与 SSE 使用 manifest `service_port` 对应的管理监听器；FPK 桌面入口必须与该监听地址保持一致。管理监听器默认绑定 `0.0.0.0:17080`，未知路径统一返回 404。
- 回环 RPC 监听器默认绑定 `127.0.0.1:17081`，只注册精确的 `/jsonrpc` HTTP、WebSocket 与 CORS 预检入口；其他路径必须返回 404。该端口不得写入 manifest、`MotrixFNOS.sc` 或 fnOS 端口映射，只允许本机反向代理访问。
- 局域网 RPC 监听器默认绑定 `0.0.0.0:17082`，同样只注册精确的 `/jsonrpc`。监听器始终绑定；局域网入口关闭时所有请求返回 404，开启后只接受真实 TCP 对端位于 IPv4 RFC1918 网段的请求，不读取代理来源 Header。
- 三个监听器共享同一个 `HttpAppState`、SQLite 连接、Aria2 运行态和退出信号；任一地址绑定失败时整体启动失败，退出时只执行一次 Aria2 保存与清理。
- Aria2 的端口、secret、进程句柄、RPC ready、运行态记录和启动/停止决策由 Rust server 内部生命周期协调器统一管理；任务操作、外部 `aria2.addUri`、启动恢复和后台监控不得绕过协调器。
- 无引擎活动、metadata、在途操作或排队请求时，Aria2 按防抖策略保持停止；普通任务列表、SSE 快照、进程/RPC 状态查询不得因读取而启动 Aria2。
- 桌面入口默认仅管理员，管理员可在应用设置中切换为设备内所有用户。端口服务不提供 fnOS 登录态 Header，管理面必须使用自身的 Web 管理密码和服务端 Session，不得伪装成已接入统一网关鉴权。

FPK 应用身份与 FN Connect 短域名：

- `manifest.appname` 是 FPK 的应用身份，当前固定为 `motrix`。
- `manifest.desktop_appname` 与 `app/ui/config` 的唯一 `.url` 入口必须同时为 `motrix.Application`。
- `manifest.desktop_applaunchname` 必须保留为空。指定 `motrix.main` 等自定义入口会使 FN Connect 生成带后缀的域名。
- 以上组合已在 fnOS 实机验证，应用可通过 `motrix.<account>.fnos.net` 打开。它不依赖反向代理或 `config/resource` 的特殊网关字段。
- 这是新的 FPK 应用身份。旧 `motrix.fnos` 安装不会按普通升级自动迁移，发布前需明确安装、数据保留和回滚策略。

升级不兼容约定：

- 既有公网 JSON-RPC 继续使用回环专用监听器 `127.0.0.1:17081` 和原 Token；局域网客户端使用 `17082` 和独立局域网 Token，不能复用管理监听器或跨入口复用 Token。
- Web 管理首次使用必须设置独立管理密码；升级不会把 JSON-RPC Token 复用为 Web 密码，也不会根据 fnOS 登录态自动放行。

## 4. 分层职责

| 层级 | 职责 | 不承担 |
| --- | --- | --- |
| FPK 打包层 | manifest、权限、图标、Web 入口、服务脚本、产物组装 | 下载业务、页面交互、业务数据持久化 |
| Rust server | 任务生命周期、配置校验、路径安全、Aria2 生命周期协调、进程与 RPC、SQLite、日志、HTTP/SSE | 页面布局、组件交互、前端临时状态 |
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
- 组件样式与组件同目录维护：`Component.vue` 的 scoped 样式放在同目录 `Component.css`，Vue 文件只保留 `<style scoped src="./Component.css"></style>` 声明；迁移时不得改变选择器、声明或视觉表现。
- `src/styles/` 只保存全局 token、基础重置、弹窗公共样式和移动端基线；业务组件样式不得重新集中堆入全局样式文件。
- UnoCSS 作为构建期原子布局补充，当前仅允许使用通过试点验证的静态 utility safelist；不得启用全局 preflight/reset、默认 extractor、attributify、shortcuts 或图标 preset。主题 token、语义 class、复杂响应式、`:deep()`、伪元素和状态样式继续使用外部 scoped CSS。

## 6. 后端约束

目标目录：

```text
server/
  src/
    main.rs
    app/
    state/
    api/
    auth/
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
- `auth/` 负责 Web 管理密码、服务端 Session、CSRF 校验、登录限速和认证中间件；鉴权状态不得并入下载设置 `AppConfig`。
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
  -> HTTP client（Web Session + 写操作 CSRF）
  -> 管理监听器 Axum Route
  -> Rust Service / Repository
  -> Aria2 JSON-RPC / SQLite
```

任务创建约束：

- 单链接、批量 URL、磁力和种子入口可以在 API / service 层分流，但必须复用统一的校验、Aria2 option 映射、内存写入和 SQLite 持久化链路。
- 批量 URL 按独立任务逐条创建，允许部分成功；单条失败不回滚已创建任务。
- 磁力任务必须先在应用私有目录解析 metadata，待前端确认文件后，才能在用户授权目录创建真实任务及其专属子目录。解析临时目录必须与任务记录关联，以支持恢复和定向清理。
- 种子任务与确认后的磁力任务都使用任务专属目录保存下载产物和 Aria2 元数据。删除文件时只能删除该任务专属目录或应用私有临时目录，不得删除授权目录根，也不得根据用户输入拼接任意删除路径。
- 回收站只保存已移出 Aria2 的任务记录；恢复任务必须重新加入 Aria2，并统一以暂停状态回到正常任务列表。删除时保留的文件用于续传，已删除的文件从头下载。
- 已完成任务重新下载时，必须先按原来源创建暂停的 Aria2 任务并持久化新 GID，再把旧文件原子移动到同一文件系统的临时目录；新任务恢复成功后才清理暂存文件。任一步失败必须移除新 GID、恢复旧任务快照和原文件，不得在新任务可靠建立前直接删除用户文件。
- BT 任务用于恢复的源种子 metadata 必须按任务 ID 保存在应用私有目录，不能依赖用户下载目录长期存在。永久删除回收站记录时同步清理私有 metadata，但不得额外删除用户下载文件。
- 磁链缺少已保存 metadata 时允许重新解析并再次进入文件确认流程；升级前已删除且源 metadata 已丢失的种子任务必须明确拒绝恢复，不得生成不可用任务。
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
- 管理监听器、回环 RPC 监听器与局域网 RPC 监听器必须在业务服务就绪后共同启动；任一监听器启动失败都不得留下半可用进程。
- PID 运行态记录必须包含进程启动时间；停止服务前必须确认 PID 仍属于当前 server 实例。
- 后端启动时准备数据目录、初始化 SQLite；Aria2 仅在恢复工作或其他引擎活动需要时按需启动或连接。
- 后端停止时保存任务状态、保存 Aria2 session、停止当前服务管理的 Aria2 实例。
- Aria2 日常空闲停止不复用完整应用退出的任务暂停语义；手动停止在有活动任务或在途操作时返回冲突，只有确认 session 保存和进程退出后才清除运行态。
- 显式手动启动（`POST /api/aria2/start`）属于生命周期操作，即使当前没有任务也允许启动 Aria2；普通读取接口仍不得因查询唤醒引擎。
- Aria2 仅在有效任务、metadata 解析、BT 活动、引擎操作或排队请求需要时保持运行；确认等待、暂停、完成、错误和回收站任务在没有有效 GID 或在途操作时允许停止。
- 自动停止必须由协调器二次确认空闲，依次完成状态持久化、session 保存、进程退出确认和运行态清理；停止期间的新请求必须取消停止、等待重启或收到明确可重试错误。
- 手动停止遇到活动任务或在途操作时返回冲突，不隐式暂停任务；完整应用退出的暂停和持久化收尾不复用日常空闲停止语义。
- Aria2 session 只作为恢复输入，SQLite 任务和操作记录仍是长期事实；未知 RPC 结果、未知 GID、暂存文件或 metadata 缺失时保留用户文件并转人工处理。
- 前端页面关闭、刷新或重新进入不等于应用退出。
- SQLite、Aria2 session、Aria2 log 和运行态文件必须放在 FPK 应用数据目录。
- 下载目录不能写死桌面用户目录，必须使用 fnOS 可访问目录或应用数据目录下的默认下载区。
- Aria2 RPC secret 只能由服务端生成和持有，不暴露给前端。
- Web 管理密码使用 Argon2id 和随机 salt 保存不可逆哈希；明文密码、密码哈希、Session ID 与 CSRF Token 不得通过普通设置接口返回或写入日志。
- 管理 API 与 SSE 默认要求有效的服务端 Web Session；管理写操作还必须校验 CSRF Token。首次启动必须完成密码初始化，关闭管理保护必须验证当前密码并使已有 Session 失效。
- 登录限速默认使用管理 listener 注入的真实对端 IP。只有对端 IP 命中 `MOTRIX_TRUSTED_PROXY_IPS`（逗号分隔的可信代理 IP allowlist）时，才读取 `X-Forwarded-For` 的第一个合法 IP；未配置或未命中时忽略该 Header。
- 会话 Cookie 的 `Secure` 属性由 `MOTRIX_WEB_COOKIE_SECURE` 显式控制，默认关闭。反向代理已终止 HTTPS 时才设置为 `true`；server 不根据客户端可伪造的 `X-Forwarded-Proto` 自动判断。
- 公网 JSON-RPC Token、局域网 JSON-RPC Token 与 Web 管理密码是三套独立凭据。JSON-RPC 写操作按入口校验对应 Token，关闭 Web 管理保护不得影响 RPC 鉴权。
- 公网 JSON-RPC 反向代理只能指向回环 RPC 专用监听器；不得依赖来源 IP、`Host`、`X-Forwarded-For` 或其他客户端可伪造 Header 区分管理面与公网 RPC 面。
- 局域网 JSON-RPC 入口只按 TCP 真实对端判断 RFC1918 IPv4 来源；回环、公网、链路本地与 IPv6 来源均不得通过，也不得通过 `X-Forwarded-For` 扩大允许范围。
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
- 日常提交只执行版本一致性、暂存区空白和 Rust 格式等快速静态检查；分支推送前对该批业务源码执行唯一一次完整测试与构建验证。
- Release 只允许修改受发布白名单约束的版本文件和 CHANGELOG；这些发布元数据变化与已验证业务源码视为等价，不重复运行源码测试。出现白名单外改动时必须中止发布。
- Release 只构建并验证新产生的双架构 FPK、SBOM、校验和与构建证明；本地完整打包必须先执行一次完整源码验证，再构建并验收 FPK。
- 依赖安全审计使用独立的每周定时任务，不并入日常推送验证或 Release。
- 测试实现必须与业务代码物理分离，不得在 `.rs`、`.ts` 或 `.vue` 业务文件内编写测试函数、测试夹具或内联 `mod tests { ... }`。
- Rust 单元测试使用独立测试文件：模块文件只允许保留 `#[cfg(test)] mod tests;` 声明，测试实现放在对应的 `tests.rs` 或 `<module>/tests.rs`；跨模块集成测试放在 `server/tests/`。
- 前端测试使用独立的 `*.spec.ts` 文件；构建与发布脚本测试统一放在 `scripts/tests/`。
- 若本文档与实际演进不匹配，先更新本文档，再继续实现。
