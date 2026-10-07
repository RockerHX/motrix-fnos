# 自动化测试完整评估

完整执行顺序可直接打开[交互式测试流程 H5](testing-flow.html)，查看命令、耗时和自动化/人工边界。

> 评估日期：2026-10-03
>
> 评估基线：分层测试实现提交 `1337f60`、`060bcd3`、`3de3934`、`2ee1703`
>
> 目标：先盘点功能，再盘点测试，最后判断自动化测试缺口；本次实现已补齐高价值扩展测试层。

## 结论

当前项目已经有一套分层测试体系，并非只有单元测试：

- Rust 领域测试、Axum API 场景测试、SQLite 测试、进程生命周期测试和外部集成测试。
- Vue/TypeScript 的 service、store、composable 和组件测试。
- Node 构建与发布脚本测试、FPK 生命周期 Shell 场景测试、FPK 解包产物校验。
- 类型检查、生产构建和严格 Rust 编译检查。

本次实现已经补齐三类高价值自动化：

1. Playwright 独立 E2E：桌面和移动 Chromium 共 8 项冒烟测试。
2. Linux 真实 Aria2 Next sidecar：本地 HTTP fixture、RPC、下载、暂停/继续、session 恢复和异常退出。
3. 文件系统故障矩阵：文件暂存、回滚、清理 worker、删除、恢复和重新下载调用链。

仍需保留真实设备验收：真实 fnOS/FPK 安装、升级、回滚、卸载、FN Connect、共享目录授权和 App WebView。这些不能由本地浏览器或 Linux fixture 完全替代。

因此，项目现在采用分层策略：`verify` 和 `build:fpk` 保持快速；`verify:extended` 和 `build:fpk:release` 执行扩展测试；fnOS 设备测试单独作为发布前验收。

## 验证基线

本次实际执行了 `rtk pnpm run verify`：

| 阶段 | 结果 |
| --- | ---: |
| 构建与发布脚本 | 66 项通过 |
| FPK 进程身份、孤儿进程、停止卸载、就绪脚本 | 4 组通过 |
| Rust 测试 | 540 项通过，5 个测试目标 |
| Rust 编译（warnings as errors） | 通过 |
| 前端单元测试 | 98 个文件、459 项通过 |
| Vue 类型检查与生产构建 | 通过 |
| Playwright E2E | 8 项通过（桌面/移动 Chromium） |
| 文件系统故障与任务调用链 | 38 项通过 |
| Aria2 sidecar | macOS 跳过，Linux CI 执行真实二进制 |

此前同一工作区执行 `rtk pnpm run build:fpk`，双架构 FPK（x86_64、ARM64）构建和 `verify:fpk` 解包验收也通过。`pnpm run verify` 本身不构建或解包 FPK；这是 `build:fpk` 额外完成的层级。

## 功能清单

| 功能域 | 功能范围 |
| --- | --- |
| Web 管理鉴权 | 安装向导初始化、本机隐藏输入重置与取消、登录、登出、改密、JWT、认证版本、登录限速、哈希并发限制、诊断下载 |
| 应用与就绪 | 应用信息、后端 ping、就绪探测、版本更新检查、双栈管理 listener |
| Aria2 生命周期 | 启动、停止、状态、RPC 状态、按需启动、空闲停止、session 保存、停止排空、进程归属校验 |
| URL 下载 | 单 URL、文件名和输出路径校验、保存目录授权、暂停/继续、限速和连接数 |
| 批量下载 | 逐 URL 创建、部分成功、部分失败、批量代理预检 |
| BT 下载 | multipart 上传、任务专属目录、metadata 保存、暂停、恢复、重新下载 |
| 磁力下载 | metadata 解析、跟随 GID、文件确认、选择文件、确认前禁止继续、metadata 缺失恢复 |
| 任务管理 | 列表、状态筛选、详情、文件上下文、错误状态、SSE 快照、任务监控 |
| 回收站 | 删除、保留文件、删除文件、永久删除记录、恢复、清空、部分失败和操作对账 |
| 任务代理 | 应用代理、兼容任务私密代理、任务开关、运行时切换、恢复/重下载继承、敏感信息脱敏 |
| 存储授权 | fnOS 共享目录刷新、快照持久化、路径显示转换、授权撤销、目录安全校验 |
| JSON-RPC | HTTP、WebSocket、OPTIONS、Token 隔离、`aria2.addUri`、`getVersion`、`getGlobalOption`、multicall |
| SSE | 初始快照、任务快照、退出事件、JWT 失效、断线重连、连接代际和 revision 基线 |
| 设置 | 默认目录、语言、并发、连接数、分片、超时、重试、下载代理、JSON-RPC Token、LAN RPC 开关和轮换 |
| 诊断与日志 | 调试日志列表/清理、Aria2 日志模式、日志轮转、日志占用、存储占用、诊断 ZIP、敏感信息脱敏 |
| 数据库 | schema 迁移、任务/设置/历史/错误/操作记录、备份、完整性检查、历史清理 CLI |
| FPK 交付 | manifest、权限、端口、图标、生命周期脚本、双架构 server/sidecar、产物命名、空运行数据目录 |
| Web UI | 鉴权门禁、任务列表和分类、创建/操作弹窗、设置、诊断、帮助、关于、扩展占位、桌面/移动布局 |
| 平台集成 | fnOS 文件 API、FN Connect 入口、真实共享目录、桌面入口、App WebView、手机浏览器 |

## 现有测试清单

### Rust 后端

Rust 共有 49 个测试模块文件，另有 2 个 `server/tests/` 外部集成测试文件。当前测试按领域分布如下：

| 领域 | 测试数 |
| --- | ---: |
| API（管理、鉴权、任务、设置、存储、JSON-RPC、SSE、诊断） | 140 |
| 任务与任务服务 | 166 |
| Runtime 生命周期、监控、关闭、文件清理、进程对账 | 65 |
| 数据库与迁移 | 29 |
| Aria2 配置、RPC、日志模式 | 24 |
| 认证服务、JWT、密码、限速 | 22 |
| 存储与共享目录 | 20 |
| App 启动编排 | 23 |
| 调试日志 | 11 |
| fnOS API 客户端 | 10 |
| 诊断 | 10 |
| 设置 | 11 |
| 状态与其他集成目标 | 9 |
| 合计 | 540 |

代表性测试文件包括：

- [`server/src/api/tasks/tests.rs`](/Users/rockerhx/Desktop/GitHub/motrix-fnos/server/src/api/tasks/tests.rs)：任务 HTTP 入口和授权边界。
- [`server/src/api/jsonrpc/tests.rs`](/Users/rockerhx/Desktop/GitHub/motrix-fnos/server/src/api/jsonrpc/tests.rs)：HTTP/WebSocket、多入口 Token、multicall、连接资源和 LAN 权限。
- [`server/src/tasks/service/tests.rs`](/Users/rockerhx/Desktop/GitHub/motrix-fnos/server/src/tasks/service/tests.rs)：创建、控制、代理、恢复、重下载、回滚和对账。
- [`server/src/runtime/lifecycle/tests.rs`](/Users/rockerhx/Desktop/GitHub/motrix-fnos/server/src/runtime/lifecycle/tests.rs)：生命周期转换、取消代际和资源收敛。
- [`server/tests/idle_file_stability.rs`](/Users/rockerhx/Desktop/GitHub/motrix-fnos/server/tests/idle_file_stability.rs)：只读请求和空闲监控不产生运行态文件变化。

### 前端

Vitest 使用 jsdom，当前有 98 个 spec 文件、459 项测试，覆盖：

- 认证组件、认证 service、认证 store、跨标签页通信。
- HTTP 错误和认证代际、SSE 解析和重连。
- 任务 store、任务 service、创建表单、批量操作、分类、分页、代理、恢复/删除/重新下载交互。
- 设置、代理、JSON-RPC Token、LAN RPC、诊断、日志、关于、帮助和更新检查。
- 任务表、任务卡、操作菜单、弹窗、状态徽章、进度、移动布局和主窗口编排。

测试文件示例：

- [`src/features/tasks/stores/taskStore.spec.ts`](/Users/rockerhx/Desktop/GitHub/motrix-fnos/src/features/tasks/stores/taskStore.spec.ts)
- [`src/features/auth/stores/authStore.spec.ts`](/Users/rockerhx/Desktop/GitHub/motrix-fnos/src/features/auth/stores/authStore.spec.ts)
- [`src/services/runtimeEvents.spec.ts`](/Users/rockerhx/Desktop/GitHub/motrix-fnos/src/services/runtimeEvents.spec.ts)
- [`src/views/MainWindow.spec.ts`](/Users/rockerhx/Desktop/GitHub/motrix-fnos/src/views/MainWindow.spec.ts)

### 脚本、FPK 与构建

`scripts/tests/` 有 21 个 Node 测试文件、66 项测试，覆盖：

- 版本、CHANGELOG、发布准备、SBOM、动作 SHA 固定和依赖审计。
- FPK 身份、产物、构建预检和静态架构约束。
- Rust 测试运行器、命令进度和测试 reporter。
- 测试代码隔离、样式架构、主题架构、日志文档和启动首屏。

`pnpm run verify` 另有 4 组 Shell 场景，验证进程身份、启动孤儿进程对账、停止/卸载收敛和就绪脚本。`pnpm run verify:fpk` 负责解包检查双架构 FPK 的内容、入口、二进制和空运行目录。

扩展测试不进入普通 `build:fpk`：`pnpm run verify:extended` 先执行一次 `verify`，再执行 E2E、sidecar 和文件系统测试；`pnpm run build:fpk:release` 调用它一次后再构建和验收双架构 FPK。

## 覆盖矩阵

状态含义：

- **A：自动化充分**：当前层级已有正常、异常或安全边界测试。
- **B：自动化部分覆盖**：核心逻辑有测试，但仍缺少真实依赖、组合流程或关键失败注入。
- **M：必须保留实机/人工**：当前环境无法可靠替代真实 fnOS、WebView、视觉或设备行为。
- **G：明显自动化缺口**：可以在本地或 CI 稳定自动化，但目前没有对应测试。

| 功能域 | 当前状态 | 已有证据 | 评估结论 |
| --- | --- | --- | --- |
| Web 管理鉴权 | A | Rust API/auth tests、AuthGate specs、Playwright E2E | 初始化、JWT、限速、并发哈希、旧 token 失效和真实浏览器 setup 链路均有回归 |
| 应用信息与就绪 | B | `api/app/tests.rs`、`api/tests.rs` | payload 和就绪状态有测试；更新检查的真实 HTTP handler/网络失败注入不足 |
| Aria2 生命周期 | A/B | `runtime/*/tests.rs`、`app/tests.rs`、`scripts/extended/aria2-sidecar.test.mjs` | 编排和真实 sidecar 基础矩阵已有；BT/磁力、公网网络和长期资源压力仍需扩展 |
| URL、批量、BT、磁力任务 | B | `api/tasks/tests.rs`、`tasks/service/tests.rs`、`tasks/tests.rs` | API 和业务编排覆盖强；真实引擎、真实网络、BT/磁力 metadata 仍缺端到端验证 |
| 任务控制、回收站、恢复、重下载 | A | `tasks/service/tests.rs`、`tasks/files/tests.rs`、`task_operation_reconcile/tests.rs`、`test:filesystem` | 正常链路、失败回滚、清理重试、目标冲突和调用链均有自动化；跨进程崩溃仍由发布验收补充 |
| 任务代理 | A/B | `settings/proxy/tests.rs`、API/任务 service tests、前端 proxy specs | 代理来源、继承、轮换、脱敏覆盖较完整；缺真实 Aria2 连接切换验证 |
| 路径授权与撤权 | A/B | `storage/*/tests.rs`、API tasks tests、任务 service tests | 路径包含、撤权前置检查和私有 metadata 有测试；fnOS 真实授权撤销和 OS 权限差异只能实机确认 |
| SQLite、迁移、CLI | A | `database/*/tests.rs`、`server/tests/database_cli.rs` | 迁移、完整性、备份和历史清理已有自动化 |
| JSON-RPC HTTP/WS/multicall | A/B | `api/jsonrpc/tests.rs`、`api/tests.rs` | 协议、Token 隔离、消息大小、连接上限和 LAN 权限已有测试；真实外部客户端兼容矩阵未自动化 |
| SSE 与任务实时状态 | A | `api/events/tests.rs`、`runtimeEvents.spec.ts`、`taskStore.spec.ts`、Playwright E2E | SSE 解析、失效、重连、revision、迟到响应和浏览器连接重连均有回归 |
| 设置与运行时应用 | A/B | `api/settings/tests.rs`、`settings/service/tests.rs`、前端 settings specs | 失败语义和 Token/代理设置有测试；设置 service 分层仍是未来事项，复杂 Aria2 运行态组合还可加强 |
| 日志与诊断 | A | `debug_logs/tests.rs`、`diagnostics/*/tests.rs`、API tests、前端 specs | 脱敏、轮转、占用、ZIP 和并发生成已有测试 |
| fnOS 开放 API | B/M | `fnos/tests.rs`、`api/storage/tests.rs` 的 Unix Socket mock | 协议解析和失败回退可自动化；真实 fnOS socket、Token 注入和权限目录必须实机 |
| FPK 静态产物 | A | `scripts/tests/fpk-*`、`verify:fpk`、双架构构建 | 包内容、架构、入口、权限和运行残留已有自动化 |
| FPK 生命周期脚本 | B | 4 组 Shell 测试 | 进程归属、信号升级、孤儿回收、就绪有模拟测试；真实 fnOS 生命周期语义仍需设备验证 |
| FPK 安装/升级/回滚/卸载 | M/B | `docs/installation-regression-test-plan.md`，部分 Shell 场景 | 已有详细人工计划，静态和脚本部分可自动化；真实应用中心、错误架构、升级失败和回滚仍需 fnOS |
| 前端业务组件 | B | 98 个 jsdom spec 文件 | 组件级覆盖广；`TaskActionsContainer`、`TaskFileConfirmCoordinator`、`MainWindowDialogs` 等组合组件缺直接 spec |
| 前端 service/adapters | B/G | 多数通过 store 测试间接覆盖 | `services/aria2.ts`、`backend.ts`、`storage.ts`、`aboutService.ts`、`debugLogService.ts` 缺少直接请求契约测试；部分是薄包装，优先级中等 |
| 响应式布局与 WebView | B/M | jsdom/mobile composable 测试、Playwright 桌面/移动 viewport | 真实浏览器 viewport 已自动化；FN Connect/App WebView、文件选择和视觉验收仍需设备 |
| 发布与依赖审计 | A/B | scripts tests、独立 weekly audit workflow | 脚本和锁文件流程可自动化；依赖审计不是每次 verify 的步骤，而是独立定时任务 |

## 自动化缺口

### 高价值、可以自动化

1. **API handler 契约补齐**：给 `app/update-check` 的网络成功/失败、logout 的 204、任务 file-context/confirm/proxy/control 全入口建立显式 handler 表格测试；已有 service 测试不能完全代替 HTTP 状态和错误码校验。
2. **前端薄 service 与组合组件**：为 `services/aria2.ts`、`backend.ts`、`storage.ts`、`aboutService.ts`、`debugLogService.ts` 和未直接测试的任务确认/操作协调组件补最小请求方法及状态编排测试。
3. **覆盖率报告**：为 Rust 和 Vitest 增加可选覆盖率采集与最低门槛。当前没有覆盖率配置，无法用数字识别“测试通过但代码分支未触达”的区域；门槛应按领域逐步建立，不能一开始设一个虚高总百分比。

### 必须保留真实设备/人工

- fnOS 应用中心安装、正确/错误架构安装行为。
- `install_*`、`upgrade_*`、`uninstall_*` 与真实应用数据保留、失败关闭、回滚行为。
- FN Connect 域名、桌面入口、真实端口映射、IPv4/IPv6、防火墙和反向代理。
- fnOS 共享目录授权、撤权、`TRIM_API_TOKEN`、官方 Unix Socket 和目录显示 API。
- 飞牛 App WebView 的主题、文件选择、宿主 SDK、前后台切换和网络恢复。
- 真实设备上的 x86_64/ARM64 安装兼容性、资源限制和长时间运行稳定性。
- 视觉间距、字体、颜色、滚动、触控和不同浏览器/WebView 的实际呈现。

这些场景仍然可以自动采集日志、端口、进程、HTTP 响应和文件快照；“需要人工”不等于每次都靠肉眼或手工记录。

## 测试流程方面的发现

- 本地 `verify` 是完整源码门禁，GitHub `Verify` 在 `main`/`develop` 的 push 和 PR 上自动运行；nightly 或手动 dispatch 运行 `verify:extended`。
- `verify` 不包含 FPK 构建和解包；`build:fpk` 才串联完整源码验证、双架构构建和 `verify:fpk`。发布 workflow 按当前架构决策只构建新产物，不重复源码验证，因此需要保证发布前已有同一提交的 verify 证据。
- 当前仍没有真实 fnOS 测试和代码覆盖率门槛。测试数量很高，但不能据此推导功能覆盖率百分比；真实设备验收和覆盖率应继续单独管理。
- `docs/installation-regression-test-plan.md` 已经把主要实机用例列全，下一步重点是把其中可在临时目录、mock fnOS 或本地 sidecar 完成的部分拆回自动化脚本。

## 评估结论

现有单元、服务级和扩展自动化基础已经覆盖主要可自动化功能；剩余缺口集中在 API 契约细化、前端薄 service、真实 fnOS/FPK 设备和可量化覆盖率。

推荐后续顺序：

1. 补齐剩余 API handler 契约和前端薄 service 测试。
2. 将 FPK 静态/生命周期测试继续扩展为可在 CI 运行的安装模拟测试。
3. 建立 fnOS x86/ARM 实机验收自动采集和覆盖率门槛。
