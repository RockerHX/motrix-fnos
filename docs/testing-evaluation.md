# 自动化测试完整评估

本文记录测试覆盖与自动化/人工边界，执行命令见[开发命令说明](development-scripts.md)。

> 更新日期：2026-10-27（原评估日期：2026-10-03）
>
> 历史验证基线：`42c8eaf`；固定 Aria2 Next 2.8.6。下方基线表保留当时的执行结果；覆盖说明随代码维护，历史数量不代表当前测试总数。
>
> 用途：维护覆盖矩阵、自动化缺口与真实设备验收边界。

## 结论

现有 Rust 领域/API/SQLite/进程测试、Vue service/store/composable/组件测试、构建发布脚本与 Shell 场景、类型检查和生产构建覆盖主要可自动化功能。剩余缺口集中在 API 契约细化、前端薄 service 与组合组件、可量化覆盖率，以及真实 fnOS/FPK 验收。

扩展层包含桌面/移动 Chromium E2E、Linux 真实 sidecar 和文件系统故障矩阵：浏览器使用模拟 API；sidecar 覆盖 HTTP fixture、RPC、下载、暂停继续、session 恢复及异常退出；文件系统覆盖暂存、回滚、后台清理、删除、恢复及重新下载。它们不能替代真实 fnOS、WebView、共享目录和设备行为。

验证入口及组合关系见[开发命令](development-scripts.md#测试与验证)，当前覆盖证据见下方矩阵。

## 验证基线

截至 `42c8eaf` 的最近一次完整源码验证为 `RUSTFLAGS=-Dwarnings pnpm run verify`，记录如下；固定引擎安全专项对应 `e08d5a4`，后续提交只纠正发布目标版本：

| 阶段 | 已有结果与边界 |
| --- | --- |
| Rust / 前端 / 脚本 | 分别 617 / 465 / 91 项通过；Rust 另有 1 项固定候选测试默认 ignored，已单独执行通过 |
| Shell、严格编译、类型检查与构建 | 完整源码验证通过；不等于 FPK 或实机验收 |
| 浏览器冒烟 | 该基线桌面/移动 Chromium 共 14 项通过，覆盖登录、创建、JWT 失效、SSE 重连及实时进度；API 由 fixture 模拟，不连接真实 Rust server 或引擎 |
| 原生引擎安全与恢复组合 | macOS ARM64：12 通过、1 个异常 Content-Range 普通暂停失败；安全强停/文件保全通过，上游缺陷未修复 |
| 真实 Rust 管理 handler → 引擎 RPC → 异常 HTTP 源 | 固定候选专项单独通过；未走 TCP 管理 listener、浏览器或 SSE |
| 双架构候选 FPK | 已构建、解包并核对 SBOM、校验和及实际引擎字节；当前非正式发布产物 |
| Linux / fnOS | ARM64 早期隔离引擎核心场景已有结果；新增 Linux 安全场景、真实应用安装/升级/回滚及 x86 实测仍待补 |

普通 `verify` 不包含浏览器、全部真实引擎组合或 FPK 解包；不能由其通过推导这些检查通过。必要补验、已知限制及延期范围见[引擎升级待验项](future-development-plan.md#6-engine-upgrade-290motrix-200-下载引擎升级)，不再要求将长时压测或完整网络抓包作为本次门槛。

## 测试组织

- Rust 领域与 API 测试使用模块内独立 `tests.rs`，外部集成测试位于 `server/tests/`。除矩阵列出的业务测试外，[空闲文件稳定性](../server/tests/idle_file_stability.rs)验证只读请求和空闲监控不改变运行态文件。
- 前端 `*.spec.ts` 使用 jsdom，覆盖鉴权与跨标签页通信、HTTP/SSE、任务与设置编排、组件和移动布局；代表入口包括 [taskStore](../src/features/tasks/stores/taskStore.spec.ts)、[authStore](../src/features/auth/stores/authStore.spec.ts)、[runtimeEvents](../src/services/runtimeEvents.spec.ts) 和 [MainWindow](../src/views/MainWindow.spec.ts)。
- `scripts/tests/` 覆盖版本/CHANGELOG/发布、SBOM、动作 SHA 与依赖审计、FPK 身份/预检/产物、Rust runner、进度与 reporter，以及测试隔离、样式、主题、日志文档和启动首屏守卫。4 组 Shell 场景覆盖进程归属、信号升级、孤儿回收和就绪；FPK 解包检查另由 `verify:fpk` 执行。

## 覆盖矩阵

状态含义：

- **A：自动化充分**：当前层级已有正常、异常或安全边界测试。
- **B：自动化部分覆盖**：核心逻辑有测试，但仍缺少真实依赖、组合流程或关键失败注入。
- **M：必须保留实机/人工**：当前环境无法可靠替代真实 fnOS、WebView、视觉或设备行为。
- **G：明显自动化缺口**：可以在本地或 CI 稳定自动化，但目前没有对应测试。

| 功能域 | 当前状态 | 已有证据 | 评估结论 |
| --- | --- | --- | --- |
| Web 管理鉴权 | A | Rust API/auth tests、AuthGate specs、Playwright E2E | 安装初始化、本机重置、JWT、限速、并发哈希、旧 token 失效和浏览器登录链路均有回归；安装向导本身待 fnOS 验收 |
| 应用信息与就绪 | B | `api/app/tests.rs`、`api/tests.rs` | payload 和就绪状态有测试；更新检查的真实 HTTP handler/网络失败注入不足 |
| Aria2 生命周期 | A/B | `runtime/*/tests.rs`、`app/tests.rs`、`scripts/extended/aria2-sidecar.test.mjs` | 已补原生 HTTP/BT 恢复、有界退出、长连接排空、连接资源与存储故障回归；真实 Linux/设备边界另行记录 |
| URL、批量、BT、磁力任务 | B | `api/tasks/tests.rs`、`tasks/service/tests.rs`、`tasks/tests.rs` | 已有真实引擎 HTTP/BT、离线 metadata 和文件选择 fixture；浏览器到真实引擎的完整 fnOS 链路仍待验收 |
| 任务控制、回收站、恢复、重下载 | A | `tasks/service/tests.rs`、`tasks/files/tests.rs`、`task_operation_reconcile/tests.rs`、`test:filesystem` | 原 GID 保留、显式从零恢复、异步清理、回滚和对账已有自动化；真实 fnOS 重启及崩溃边界仍需验收 |
| 任务代理 | A/B | `settings/proxy/tests.rs`、API/任务 service tests、前端 proxy specs | HTTP 兼容绑定、BT 总代理、受控 peer/tracker/web-seed、切换与错误代理阻断已有回归；完整网络泄漏检查未完成 |
| 路径授权与撤权 | A/B | `storage/*/tests.rs`、API tasks tests、任务 service tests | 路径包含、撤权前置检查和私有 metadata 有测试；fnOS 真实授权撤销和 OS 权限差异只能实机确认 |
| SQLite、迁移、CLI | A | `database/*/tests.rs`、`server/tests/database_cli.rs` | 迁移、完整性、备份和历史清理已有自动化 |
| JSON-RPC HTTP/WS/multicall | A/B | `api/jsonrpc/tests.rs`、`api/tests.rs` | 协议、Token 隔离、消息大小、连接上限和 LAN 权限已有测试；真实外部客户端兼容矩阵未自动化 |
| SSE 与任务实时状态 | A | `api/events/tests.rs`、`runtimeEvents.spec.ts`、`taskStore.spec.ts`、Playwright E2E | SSE 解析、失效、重连、revision、迟到响应和浏览器连接重连均有回归 |
| 设置与运行时应用 | A/B | `api/settings/tests.rs`、`settings/service/tests.rs`、前端 settings specs | 设置领域编排已归位；设置与 Token 并发、保存失败、即时应用失败契约及代理设置有回归，复杂 Aria2 运行态组合还可加强 |
| 日志与诊断 | A | `debug_logs/tests.rs`、`diagnostics/*/tests.rs`、API tests、前端 specs | 脱敏、轮转、占用、ZIP 和并发生成已有测试 |
| fnOS 开放 API | B/M | `fnos/tests.rs`、`api/storage/tests.rs` 的 Unix Socket mock | 协议解析和失败回退可自动化；真实 fnOS socket、Token 注入和权限目录必须实机 |
| FPK 静态产物 | A | `scripts/tests/fpk-*`、`verify:fpk`、双架构构建 | 包内容、架构、入口、权限和运行残留已有自动化 |
| FPK 生命周期脚本 | B | 4 组 Shell 测试 | 进程归属、信号升级、孤儿回收、就绪有模拟测试；真实 fnOS 生命周期语义仍需设备验证 |
| FPK 安装/升级/回滚/卸载 | M/B | `server/tests/engine_upgrade.rs`、`scripts/tests/fpk-upgrade-lifecycle.test.mjs` 与 Shell 场景 | 不加载旧任务和旧操作、保留文件与设置、新代任务恢复、升级脚本重复执行和进程停服检查已有隔离测试；真实应用中心顺序与失败状态仍待验收，回滚前须手动备份 |
| 前端业务组件 | B | jsdom spec 文件 | 组件级覆盖广；`TaskActionsContainer`、`TaskFileConfirmCoordinator`、`MainWindowDialogs` 等组合组件缺直接 spec |
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

## 验证边界与后续顺序

源码、扩展测试及 FPK 验包的职责和触发方式以[开发脚本](development-scripts.md#测试与验证)为准。Release 不重复源码验证，发布前须有同一业务源码提交的 verify 证据。当前没有真实 fnOS 测试和覆盖率门槛，测试数量不能推导覆盖率百分比。

必要 Linux/真机补验及延期范围以[引擎升级待验项](future-development-plan.md#6-engine-upgrade-290motrix-200-下载引擎升级)为准；已有自动化不重复转成手工测试，延期项目不记为通过。

推荐后续顺序：

1. 补齐剩余 API handler 契约和前端薄 service 测试。
2. 将 FPK 静态/生命周期测试继续扩展为可在 CI 运行的安装模拟测试。
3. 建立 fnOS x86/ARM 实机验收自动采集和覆盖率门槛。
