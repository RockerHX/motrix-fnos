# 全仓冗余与可维护性审计（2026-10-25）

> 审计基线：`feature/2.0.1`，HEAD `64c861cd85f31f983438ed9252054299f351b80c`。
> 执行开始工作区干净；分支相对本地记录的 origin 同名分支领先 3 个提交，未执行远程同步。
> 交付范围：仅新增本报告，未修改业务代码、测试、配置、架构或接口；未打包发布，未操作真实 NAS。
> 本文是有日期的审计快照，不替代架构、接口契约和后续开发计划。

## 1. 总体判断

本项目是供 fnOS 使用的下载管理应用：Rust server 托管 Vue Web UI、管理 SQLite 状态，并控制固定版本的 Aria2 Next sidecar；管理面、回环 RPC 和局域网 RPC 分别使用独立入口及凭据。评估按个人 NAS 单实例、少量管理员/浏览器标签页、少量同时下载、历史任务持续积累的负载进行，不引入多租户或分布式系统的额外要求。多文件 BT 和数百条历史任务属于需要兼顾的正常使用方向，不据此虚构吞吐指标。

**整体结构可继续维护，不建议推倒重写。** 桌面和移动端共享业务状态，任务操作已分领域，生命周期和文件安全具备明显的边界与补偿机制。实际维护负担集中在仍可运行的双代引擎路径、任务监控的重复持久化、部分业务编排位置、重复测试夹具和局部 UI 副本。

最先处理的是 **AUD-01：普通设置与公网 RPC Token 并发保存会互相覆盖**。该问题已调用真实项目函数和 SQLite 复现；测试全绿没有覆盖这一交错。其次应降低后台重复写入，并整理已确认的冗余。异常 Content-Range 暂停失败是项目已记录的上游限制，本次再次复现，不能称为新发现或已经修复。

本报告列出 13 项主要发现。相似小问题合并说明；没有把文件长度、测试数量、空行或个人命名偏好当成问题，也没有凑足 20 项。

### 1.1 基线规模与清单口径

以 `git ls-files` 的 **588 个跟踪文件**为完整边界，包含锁文件、二进制和品牌源文件；不包含 node_modules、dist、target、.codegraph 和本地运行数据。完整路径清单见附录。下列类别互不重叠，行数包含注释及空行，仅用于说明规模。

| 类别 | 文件数 | 文本行数/说明 |
| --- | ---: | --- |
| Rust 业务源码 | 108 | 27,583 行；排除独立测试与 test_support |
| 前端业务源码、类型和样式 | 177 | 16,647 行；排除 spec、src/test 和 taskActionTestFixtures |
| 测试、测试支持与夹具 | 197 | 48,660 行；包括 Rust、Vitest、Node、Shell、浏览器及真实引擎夹具 |
| 构建、版本、发布、维护、验证脚本 | 30 | 3,378 行；排除上行测试脚本和夹具 |
| FPK cmd 生命周期脚本 | 14 | 963 行 |
| docs 现有文档 | 9 | 3,220 行；不包含本报告 |
| 其他配置、工作流、元数据、锁文件及资源 | 53 | 不把二进制折算为代码行数 |
| 合计 | **588** | 规模不等同于冗余程度 |

根目录分布为：`src 278 / server 174 / scripts 61 / packaging 24 / assets 12 / docs 9 / .github 9 / .githooks 2 / e2e 1 / public 1 / 根级文件 17`。因此不存在用少数大文件代表全部仓库的清单遗漏。

### 1.2 方法与覆盖等级

先通过 CodeGraph 确认模块、符号和调用关系，再补查其不覆盖的动态引用、Vue 模板、配置、脚本、测试和文档。判定无生产调用前同时检查测试与字符串入口；没有把“只有一个生产实现”直接等同于可删除。

覆盖等级：

- **深入**：沿指定业务路径检查输入、状态、外部副作用、持久化和失败补偿，并核对相关测试；不代表该目录每行均已人工阅读。
- **定向**：检查结构、入口及与审计问题相关的实现和测试；其余文件只纳入清单和验证。
- **清单/静态**：确认用途、引用或配置关系；没有实际运行平台或服务。

| 范围（文件数含各自测试） | 覆盖 | 已检查重点及结论边界 |
| --- | --- | --- |
| server/src/api（28）、app（3） | 深入核心路径 | 路由、状态装配、鉴权/设置/任务/RPC/SSE、启动恢复；没有穷举每个 HTTP 契约组合 |
| server/src/tasks（43） | 深入 | 创建、控制、磁链确认、恢复/重下、代理、删除、路径、操作记录和 repository；metadata 二进制解析分支以定向阅读和现有测试为主 |
| server/src/runtime（24） | 深入 | 生命周期、monitor、reconcile、退出预算、文件清理、引擎版本解析与启停；Linux 特有分支未在目标系统执行 |
| server/src/database（13）、settings（5） | 深入 | 整行配置写入、任务/文件快照、迁移事务、引擎代际、代理意图；并发配置另做真实数据库复现 |
| server/src/auth（9）、state（2） | 深入主要边界 | Argon2id、哈希并发槽、JWT/版本、真实对端/可信代理、状态所有权；不是密码学或全面渗透测试 |
| server/src/aria2（17）、config（2） | 定向 | RPC 请求/错误、原生与兼容选项、全局配置、secret/state-dir；固定候选另行实测 |
| server/src/storage（6）、fnos（2） | 定向 | 授权快照、目录归属、路径与 Unix Socket 客户端；真实 fnOS SDK/API 行为保留实机边界 |
| server/src/diagnostics（5）、debug_logs（2） | 定向 | 日志/诊断出口、敏感数据、资源统计；未做全部导出内容的独立攻击测试 |
| server/src 根级入口与测试支持（4）、server/tests（7） | 定向＋执行 | CLI、测试支持、集成测试用途；全部默认测试执行，ignored 原生安全测试单独执行 |
| src/features/tasks（93） | 深入主要链路 | store/service/composable、桌面/移动操作、回收站和确认弹窗；展示组件与 CSS 定向检查 |
| src/features/auth（13）、settings（32） | 深入主要链路 | 登录代际、凭据、配置缓存、代理/RPC 设置与请求边界 |
| src/features/diagnostics（38）、about（9）、extensions（2）、help（3） | 定向 | 诊断导出/分类、更新与帮助入口；外部更新网络服务未实际联调 |
| src/services（11）、views（8）、app（12） | 深入关键编排 | HTTP、认证 SSE、订阅/刷新代际、主窗口编排；其余薄适配定向检查 |
| src/components（25）、layouts（9）、styles（4）、i18n（4）、types（7）、src 根级（4）、src/test（4） | 定向＋静态 | 组件职责、样式引用、翻译副本、类型和构建入口；未开展逐屏视觉验收 |
| scripts（61）、六个 .github/workflows、两个 hook | 定向＋执行 | 验证层级、构建复用、版本/发布白名单、保留策略；外部发布服务和 Actions 未触发 |
| packaging（24） | 静态＋Shell 模拟执行 | start/stop/status、身份与超时、安装升级卸载边界；不等于 fnOS 应用中心验收 |
| assets（12）、public（1）、根级配置/文档和 .github 其余文件 | 清单/静态 | 固定引擎资源、哈希/构建入口、设计源、依赖/忽略边界；没有新打包或生成 SBOM |
| docs 全部 9 份 | 定向交叉核对 | 职责、命令、升级语义、现有计划和测试历史；本地文件链接检查，不声称 API 字段、锚点及外链逐项全部一致 |

### 1.3 核心调用链覆盖

| 流程 | 已追踪路径 | 重要结论 |
| --- | --- | --- |
| 登录与鉴权 | AuthGate/authStore → authService/http → api/auth → auth 的密码、JWT、限速 → 持久化；SSE 使用认证请求 | 前端反馈、服务端校验、JWT 版本和入口隔离职责不同，应保留 |
| URL/种子任务创建 | 创建 composable → taskStore/taskService → api/tasks → TaskService create → 选项/目录/代理 → Aria2、内存、SQLite/操作记录 | 批量部分成功与单任务回滚不是可互换的重复实现 |
| 磁链确认 | metadata 私有目录 → 原生 RPC 文件状态 → 确认 → 已授权目录与选择 → 同 GID 下载 | 原生恢复和旧 followedBy 路径需要分开评价，见 AUD-03 |
| 暂停、恢复、回收站恢复 | 状态操作 → control/restore → 生命周期租约、暂停确认、原生状态 → 持久化/补偿 | 超时与不确定 RPC 结果的保守处理有必要 |
| 回收站、永久删除及重下 | delete/restore → 路径校验 → 原子暂存 → 操作记录 → file_cleanup/reconcile | 不能为减少代码跳过暂停确认、授权复核、暂存和重启恢复 |
| 代理切换 | 设置/任务代理 → 私密绑定、proxy service → 引擎选项 → SQLite，失败补偿/未知结果对账 | HTTP 任务意图、BT 总开关和私密兼容覆盖各有职责，不宜合成一个布尔状态 |
| SSE 与刷新 | runtime event → api/events → runtimeEvents → taskStore | revision、连接代际与请求失效防护解决不同竞态；本次未确认可删除的重复请求 |
| 数据库迁移 | connect_database → initialize_schema/migrate_schema → 逐迁移事务与记录 → 代际筛选/启动恢复 | 已发布迁移及旧字段保留具有历史数据兼容价值，不应回写或合并历史迁移 |
| 引擎启停 | fnOS cmd → Rust bootstrap/协调器 → process start/stop、session、monitor、shutdown | PID/启动时间/可执行文件联合校验、总退出预算和空闲二次检查应保留 |

## 2. 按优先级排列的主要发现

优先级定义：**P1 优先修复正确性/凭据问题；P2 近期维护；P3 低风险精简或文档对齐**。除 AUD-01 外，没有把静态维护问题表述为已发生的生产事故。

### 必须优先修复

### AUD-01 · P1 · 普通设置与公网 RPC Token 共用整行 JSON，读改写会互相覆盖

**位置与证据：** [settings/service.rs](../server/src/settings/service.rs#L63) 第 63–112 行把 AppConfig 与 jsonRpcToken 共同存入 `app_config.download`。save_app_config 先读旧 Token 再覆盖整行，save_json_rpc_token 先读旧配置再覆盖整行；[database/settings.rs](../server/src/database/settings.rs#L244) 第 244–296 行提供整行 UPSERT。[api/settings.rs](../server/src/api/settings.rs#L163) 第 163–190、265–277 行在保存后各自更新缓存，没有跨两个入口覆盖完整读改写和缓存更新的共同串行边界。

**场景与实证：** 两个标签页同时保存下载设置和更新 Token。仓库外的小型 Rust 程序直接调用项目的 connect_database、save_app_config、save_json_rpc_token 和读取函数，100 次 `tokio::join!` 并发保存均先断言两次返回成功，随后发现 **43 次新 Token 丢失、55 次新并发下载数丢失**。这是本机这一次调度的计数，不是生产故障概率；未走 TCP/HTTP，也没有实测重启鉴权。

**影响：** 界面保存成功但磁盘保留旧值；内存和数据库可能不同。启动会重新读取并启用磁盘 Token，见 [app/mod.rs](../server/src/app/mod.rs#L556) 第 556–579 行，所以轮换后旧 Token 可能在重启后重新有效；这一重启后果由代码推导，不混写成已做过实机复现。

**最小改法：** 在现有 settings 领域集中两个更新入口，为读取、合并、写入及缓存发布提供同一个串行顺序；或使用能原子更新独立字段的数据库写法，同时防止旧缓存覆盖新值。仅给单次 UPSERT 加锁不够。优先不拆 schema、不改变现有公开接口；多密钥系统不是修复本问题的前提。

**兼容风险与验证：** 保留普通设置不返回 Token、空 Token 清除及失败不改变有效凭据的语义。后续补一项可控制交错的并发回归，并检查保存失败、立即鉴权和重启加载；不能只用一次随机并发成功宣告修复。

### 建议近期清理

### AUD-02 · P2 · 只要有活动任务，后台就重复保存全部同代任务与文件快照

**位置与证据：** [runtime/task_monitor.rs](../server/src/runtime/task_monitor.rs#L164) 第 164–195 行只在进入监控前判断是否存在 active/pending，刷新后对全部同代任务调用 persist_download_task_states；第 10、45 行每轮工作前等待 500ms。[database/tasks.rs](../server/src/database/tasks.rs#L365) 第 365–404 行逐任务持久化；第 107–151 行每次保存均删除并重插该任务的全部文件快照。SSE 的前后比较位于持久化之后，不能减少这些写入。

**场景与影响：** 一条 URL 正在下载时，历史中未变化的已完成 BT、暂停或回收站任务也被重复处理；多文件历史会扩大 SQL 次数、事务占用和 NAS 磁盘写入。代码路径已确认，未测量真实 NAS 延迟、写放大字节或寿命影响；循环周期包含实际工作时间，不能宣称固定每秒两次。

**最小改法：** 先区分真正变化的任务，再持久化；文件列表未变时不全量重建文件快照。先消除未变化历史的写入，再决定是否需要更细的进度节流，不引入缓存框架或修改 SSE 协议。

**兼容风险与验证：** 保留任务/文件/代理/操作记录的事务一致性，不能遗漏最后一次完成、暂停、退出快照，也不能因陈旧 monitor 快照覆盖新的操作事实。现有 [idle_file_stability.rs](../server/tests/idle_file_stability.rs) 检查的是空闲稳定性；后续增加“一个活动任务＋大文件列表的未变化历史”的 SQL 写入计数场景，再观察性能。当前没有该混合负载的定量基准。

### AUD-03 · P2 · 旧代引擎仍是可执行模式，双套任务规则的维护边界没有收口

**位置与证据：** [aria2_process/resolve.rs](../server/src/runtime/aria2_process/resolve.rs#L20) 第 20–24 行仍接受 2.5.5 和 2.8.6；[app/mod.rs](../server/src/app/mod.rs#L524) 第 524–565 行可构造 normal LegacyV1，且没有显式路径、解析不到二进制时回退到 LegacyV1；[config/aria2.rs](../server/src/config/aria2.rs#L86) 第 86–92 行还通过 state_dir 是否存在区分代际。[tasks/refresh.rs](../server/src/tasks/refresh.rs#L39)、[tasks/progress.rs](../server/src/tasks/progress.rs#L25) 中保留旧后继 GID/进度规则；[test_support.rs](../server/src/test_support.rs#L8) 的旧版本假引擎也支撑大量测试。

**场景与影响：** 官方 FPK 目标固定 2.8.6，但外部引擎路径或开发配置仍可能进入旧模式。新行为需要同时判断两套执行语义；只看正式包就把 LegacyV1 全部当死代码会破坏仍可达的入口。架构第 7.2 节要求新版不加载/继续旧代任务，当前“支持运行旧引擎”的边界需要明确。

**审核补充（2026-10-26）：** 缺失引擎回退不只是支持策略问题。调用真实启动函数的独立复现得到 `binary_missing=true generation=LegacyV1 mode=normal download_available=true visible_ids=[7]`，确认违反架构门禁。该启动缺陷应立即修复，不等待旧引擎退役。

**最小改法：** 缺失或身份不可确认的引擎统一受限管理且不加载任务。外部 2.5.5 支持边界明确后，让真实执行入口统一收口，再逐批删除旧任务执行算法和专属测试替身。保留旧代标识解析、只读识别、历史数据保全与已发布迁移。

**兼容风险与验证：** 涉及环境变量、本地开发、旧数据库及回滚，不属于可独立直接删除项。测试未知/缺失/2.5.5/2.8.6 二进制、旧代损坏数据隔离、旧文件保留、任务 ID 不复用及回滚；不能用统一改为 NativeV2 的测试夹具掩盖真实恢复差异。与 ENGINE-UPGRADE-290 重合，但该计划未授权直接删除兼容能力。

### AUD-04 · P2 · 设置业务编排仍集中在 HTTP handler

**位置与证据：** [api/settings.rs](../server/src/api/settings.rs#L163) 第 163–190 行负责保存、缓存、RPC 默认目录、日志和引擎即时应用；相关 runtime apply 编排也留在 API 文件。该问题已在 [后续开发计划](future-development-plan.md#3-future-refactor-01设置服务分层重构) 的 FUTURE-REFACTOR-01 明确登记。

**场景与影响：** 后台或非 HTTP 入口需要保存同类设置时，要重复正确的操作顺序，或依赖 HTTP 模块；service 测试不能直接覆盖完整业务语义。这也使 AUD-01 的共享更新边界更容易遗漏。

**最小改法：** 将现有编排移入 settings service，handler 保留解析、错误与响应转换。复用已有状态、生命周期和持久化方法，不建立通用业务层；可与 AUD-01 共用更新入口，但不要延后凭据修复等待大重构。

**兼容风险与验证：** 必须保持“持久化成功、即时应用失败仍保留设置”“引擎未运行不为保存主动启动”的行为，以及当前 HTTP 响应和诊断。按既有计划补 service 失败顺序测试，API 只保留必要契约测试。本报告不重复新建同名计划。

### AUD-05 · P2 · 后台永久删除通过 api 模块创建 TaskService，依赖方向倒置

**位置与证据：** [runtime/file_cleanup.rs](../server/src/runtime/file_cleanup.rs#L169) 第 169–180 行使用 `crate::api::build_task_service` 完成永久删除；工厂在 [api/mod.rs](../server/src/api/mod.rs#L95) 第 95–111 行。已核对非 API 模块中的相关引用，这个 worker 是具体的运行时反向依赖；应用入口组合路由不算同类问题。

**场景与影响：** HTTP 之外的重启清理也必须借用路由模块的依赖装配。API 文件的整理会牵动文件清理，领域与传输职责不够清楚；尚未发现因此导致删除失败的实证。

**最小改法：** 将现有 TaskService 装配函数移到应用组合位置，供 handler 和 worker 共用，保留同一组依赖；无需新增 trait、容器或包装层。

**兼容风险与验证：** 保留 for_engine repository、RuntimeGuard、代理锁和状态引用；不能只迁移“看起来需要”的参数。运行已有后台清理、永久删除、失败保留记录与重启对账测试，确认 API 契约不变。

### AUD-06 · P2 · Rust 测试重复构造基础环境，并遗留大量临时目录

**位置与证据：** [shutdown/tests.rs](../server/src/runtime/shutdown/tests.rs#L402) 第 402 行起与 [task_monitor/tests.rs](../server/src/runtime/task_monitor/tests.rs#L423) 第 423 行起分别实现 ready_state、temp_dir、子进程和 MockAria2Server；[tasks/service/tests.rs](../server/src/tasks/service/tests.rs#L3536) 的 ServiceFixture、sample_task、temp_dir 及 API 测试存在类似副本。部分目录有零散清理，基础 fixture 没有统一 Drop 清理；shutdown 测试也没有对应统一回收。

**场景与实证：** 本次 verify 后，仅筛选创建于执行时段且名称带本次 Rust 测试进程号 97349 的 `motrix-fnos-*` 临时目录，仍有 **281 个目录、387 个文件、13,707,862 字节**。这是带时间/进程过滤的运行后观察，不是全系统残留统计，也不是长期磁盘增长率；未删除这些证据目录。

**影响与最小改法：** 新增 DownloadTask 字段或基本状态时要同步多套样板；重复运行会积累测试文件。先在已有 [test_support.rs](../server/src/test_support.rs) 中放一个拥有临时目录并负责 Drop 的小工具，再集中稳定的基础任务/状态构造。参考 [runtime_stability.rs](../server/tests/runtime_stability.rs#L21) 已有的所有权清理模式；情景专属 mock 行为保留本地，不造通用 fixture 框架。

**兼容风险与验证：** 先终止子进程、关闭数据库再清理目录；失败时能保留必要诊断，不能影响并行测试。后续运行受影响用例并检查同次运行是否残留；前端已有 taskActionTestFixtures 可按需复用，不能抹掉场景关键字段来追求所有夹具相同。

### AUD-07 · P2 · verify:extended 中的文件系统阶段重复执行普通 Rust 用例

**位置与证据：** [verify-extended.mjs](../scripts/verify/verify-extended.mjs#L5) 第 5–9 行先完整 verify，最后执行 test:filesystem；[run-filesystem-tests.mjs](../scripts/verify/run-filesystem-tests.mjs#L5) 第 5–16 行只是用五个名称过滤器执行 cargo test --lib，覆盖 tasks::files、runtime::file_cleanup、delete_with_files、redownload 和 restore。普通 [run-rust-tests.mjs](../scripts/verify/run-rust-tests.mjs#L60) 第 60–65 行已执行完整 cargo test。

**场景与影响：** 本地/定时扩展验证会把相同用例再跑一遍；没有额外故障注入、feature 或新平台条件，因此增加时长但没有新增场景覆盖。它们与原生引擎、浏览器或实机测试不重合，不能把那些独立测试一起删掉。

**最小改法：** 保留 test:filesystem 作为人工定向诊断入口，从已运行 verify 的组合入口去掉重复阶段；若将来增加独立故障矩阵，再恢复有区别的组合检查。

**兼容风险与验证：** 不删除实际测试，也不改变独立命令。核对五个过滤器命中的用例均在完整测试中，检查组合入口及其脚本测试。本次已经复用 648 项 Rust 结果，未为重复计数再跑专项。

### AUD-08 · P2 · 部分脚本测试把源码写法误当成执行契约

**位置与证据：** [scripts/tests/toolchain.test.mjs](../scripts/tests/toolchain.test.mjs#L58) 第 58–59 行用 `const steps = quick ? [...] : [...]` 的精确源码结构抽取步骤；第 71–74、126–143 行还固定 helper 名称、回调名和命令数组的书写形式。

**场景与实证：** 本次在内存中仅一致重命名 verify.mjs 的 steps 标识符，未修改命令行为，原 quickSteps 正则就提取为空，原断言失败。该定向演示没有修改仓库，也没有重新运行整套测试。反过来，仅在不可达源码中保留匹配文本，也不能证明相应命令实际执行。

**影响与最小改法：** 无行为变化的整理要改测试，真正执行顺序错误未必被发现。只把用于证明运行行为的断言改为现有仓库已使用的沙箱/假可执行命令方式；版本、固定 SHA、安全禁止项和测试物理隔离等静态约束仍有价值，应保留。

**兼容风险与验证：** 不按 readFile/正则出现次数批量删除测试。后续验证“不改行为可重命名/提取函数”和“漏掉必要命令会失败”，同时保留严格的版本及安全边界守卫。

### AUD-09 · P2 · TaskActions 保留无生产调用的第三种按钮布局

**位置与证据：** [TaskActions.vue](../src/features/tasks/components/TaskActions.vue#L360) 第 360–455 行是 96 行普通文字 NSpace 布局；另有 icon-pill 和 compact 两种分支。实际入口只有 [TaskDesktopCard.vue](../src/features/tasks/components/TaskDesktopCard.vue#L20) 的 icon-pill 和 [TaskMobileList.vue](../src/features/tasks/components/TaskMobileList.vue#L43) 的 compact；TaskActionsContainer 向下转发。CodeGraph 调用关系加全树引用检查未发现生产入口使用默认第三种模式，只有组件测试主动构造。

**场景与影响：** 每增加任务操作或无障碍属性，都要维护三套按钮，其中一套没有当前产品入口；现有测试还在保护这个闲置模式。不是因为 Vue 文件长而提出删除。

**最小改法：** 删除第三分支和仅为它服务的导入，明确保留两种真实展示模式；同步调整测试默认参数，让行为断言落在真实模式。不必把桌面/移动所有模板强行合成复杂渲染器。

**兼容风险与验证：** 内部组件不是公开 HTTP 接口，但遗漏未显式传展示参数的新调用会改变界面。后续再次检查调用，测试桌面/移动两种模式的权限、loading、键盘/ARIA 和所有确认操作，再跑对应浏览器冒烟；仅在证明 CSS 选择器也无引用后删除样式。

### AUD-10 · P2 · RPC 指南仍要求“先处理旧任务”，与当前升级规则冲突

**位置与证据：** [jsonrpc-remote-access.md](jsonrpc-remote-access.md#L37) 第 37 行将 -32005 解释为升级受限并要求“须先处理旧任务”；[api-contract.md](api-contract.md#L204) 第 204–206 行、架构第 7.2 节和 ENGINE-UPGRADE-290 明确旧任务不阻止升级，旧记录与文件保留，新版不加载。

**场景与影响：** 用户遇到未知引擎或当前代状态损坏时，被引导去处理旧任务；该操作未必解除门禁，还会误解旧数据保全策略。未观察到真实用户因此删除数据。

**最小改法：** 指南只说明 -32005 对应兼容门禁，指向当前的诊断/受限原因与接口契约；移除旧任务必须先处理的要求。保留简短用户操作说明，详细规则继续由契约维护。

**兼容风险与验证：** 不改变任何运行行为和错误码。校对未知引擎、当前代损坏、旧代损坏三个分支的说明，检查文档链接；无需为这句话增加源码字符串测试。

### 可顺手精简

### AUD-11 · P3 · 恢复与重新下载确认弹窗是仅三处文案不同的副本

**位置与证据：** [TaskRedownloadConfirmDialog.vue](../src/features/tasks/components/TaskRedownloadConfirmDialog.vue) 与 [TaskRestoreConfirmDialog.vue](../src/features/tasks/components/TaskRestoreConfirmDialog.vue) 的 props、事件、代理开关及禁用/loading 结构一致，差别只在第 26、35、44 行的标题、正文和确认按钮标签。

**场景与影响：** 改遮罩关闭策略、代理开关或可访问性需要成对修改，容易只改其中一个。未发现二者现有行为已经分叉。

**最小改法：** 保留一份小型确认视图，传标题、正文和确认标签，父组件继续分别处理 restore 与 redownload；复用现有 AppConfirmDialog，不增加弹窗系统。

**兼容风险与验证：** 不能合并操作语义或共用彼此的代理临时状态；BT 不显示任务级代理、退出时关闭弹窗等行为保留。迁移两份弹窗 spec 中的关键差异及父组件事件断言，不把删除/永久删除确认并入此组件。

### AUD-12 · P3 · 已传入的任务标签又生成回退副本，诊断分类也维护两份映射

**位置与证据：** [TaskActionsContainer.vue](../src/features/tasks/components/TaskActionsContainer.vue#L79) 第 79–98 行生成任务标签；[TaskDetailsDialog.vue](../src/features/tasks/components/TaskDetailsDialog.vue#L58) 第 58–79 行再次生成同一组回退值，但唯一生产调用 [TaskActions.vue](../src/features/tasks/components/TaskActions.vue#L457) 第 457–466 行始终传 labels。另在 [DebugLogList.vue](../src/features/diagnostics/components/DebugLogList.vue#L49) 第 49–60 行与 [useDebugLogExport.ts](../src/features/diagnostics/composables/useDebugLogExport.ts#L76) 第 76–87 行重复七类日志名称映射。

**场景与影响：** 新增按钮文案或日志分类时，两处要同步；详情弹窗默认标签只为孤立挂载保留，生产路径没有使用。标签副本和分类副本均属小规模问题，不构成引入全局翻译封装的理由。

**最小改法：** 详情标签设为显式必传并移除无用回退；日志分类在 diagnostics 领域保留一份现有格式化规则供列表和导出共用。两项可以分别提交，不必耦合重构。

**兼容风险与验证：** 迁移孤立组件测试的 labels，保留未知日志分类的原始值回退及实时语言切换；不要误删仍用于其他详情文案的 useI18n。对照列表、导出和两种语言，检查无标签遗漏。

### AUD-13 · P3 · 设计文档中的 token 名称和部分值与实现脱节

**位置与证据：** [DESIGN.md](design/DESIGN.md#L94) 第 94–128 行使用 --color-surface、--color-text-primary 等名称，实际 [tokens.css](../src/styles/tokens.css#L4) 第 4–38 行使用 --app-color-*、--app-text-*；文档深色 surface 为 #142236、主文字为 #F4F8FD，实际分别为 #151515、#dce8e2。圆角也有 6/8/10 与 12/14/16/18 的口径差异。

**场景与影响：** 后续 AI 或开发者照文档添加 CSS，可能使用未定义变量，或引入另一套视觉数值；设计意图与当前实现不易区分。此项不是宣称某组颜色更好，也没有验证全部页面的视觉偏差。

**最小改法：** 先确认哪些值是当前规范、哪些是尚待实现的设计目标；文档引用真实 token 名称，明确目标/当前映射与状态，避免平行维护两份“当前值”。无需新增 token 生成器。

**兼容风险与验证：** 不能借文档清理直接把所有 UI 改色、改圆角。后续若仅对齐文档，检查变量引用即可；若另行授权视觉调整，按桌面、手机、宿主明暗主题做截图与人工验收。

## 3. 不应按“重复”直接删除的部分

| 表面上相似的实现 | 为什么仍有必要 |
| --- | --- |
| 前端输入反馈与后端参数、路径、权限校验 | 浏览器不可信；体验校验不能替代服务端安全边界 |
| TaskRepository 与 SqliteTaskRepository | 失败注入测试有替代实现；当前 trait 支撑验证补偿，不能只按一个生产实现删掉 |
| 生命周期锁、任务操作守卫、代理更新锁、退出状态 | 保护的资源/操作顺序不同；未证明删锁安全 |
| 文件暂存/还原、RPC 结果未知、操作日志、启动对账 | 处理外部副作用与持久化不原子的问题，是避免数据丢失的必要复杂度 |
| 旧 schema、历史 bootstrap 字段、引擎代际解析 | 已发布数据库迁移与保全需要；旧任务执行路径可收口，不代表旧记录可删除 |
| HTTP/BT 代理分支和私密覆盖 | 任务级意图、BT 全局规则和兼容凭据边界不同 |
| service 测试、handler 契约、浏览器和真实引擎专项 | 验证不同边界，不能按出现同名“删除/暂停”判断重复；AUD-07 是实际重复执行同一 Rust 用例 |
| 桌面卡片与移动列表 | 展示结构差异符合架构；当前共用 store/service，不应额外造统一渲染框架 |
| assets 内两种 Linux 引擎二进制与校验文件 | 分别服务不同 CPU 的正式交付；不能因 macOS 本地不执行而删除 |
| 品牌 SVG 方案及打包 PNG | [品牌资源说明](../assets/icons/logo/README.md) 已说明设计源用途，运行时未引用不等于无主垃圾 |
| UnoCSS 与 scoped CSS 并存 | 架构明确限制 safelist 试点，当前使用范围有限且有真实用途，不建议为减少一个依赖重写样式体系 |

依赖核对没有得到可以直接删除的生产依赖：例如 hyper/http-body-util 用于 fnOS Unix Socket HTTP，fs2 用于锁与存储，subtle 用于凭据比较，sha2 用于 metadata，url 用于代理规范化；前端运行依赖都有入口。openai/js-tiktoken 是发布说明生成工具的开发依赖，不能因业务页面没有 import 判死。**确认可删除依赖为 0 个**，这不是安全漏洞审计结论。

生成物边界总体明确：dist/target/运行数据库/FPK stage 被忽略，固定引擎作为有版本与校验依据的受控资源跟踪。少量已无当前业务用途的 .gitignore 历史规则没有造成行为问题，不列为主要发现，也不为了行数安排清理。

## 4. 待验证疑点与已知限制

| 事项 | 当前证据 | 下一步及为何没有升级为确认缺陷 |
| --- | --- | --- |
| monitor 快照与同时发生的任务删除/更新交错 | monitor 读取、刷新、写回跨越异步边界 | 本次只确认 AUD-02 的重复写；尚未构造确定的陈旧快照覆盖/任务复活序列，需可控调度后才能定性 |
| 全量快照的实际 UI/SSE/SQLite 性能上限 | 全量 task/file 数据和 500ms 等待存在，但未做持续负载测量 | 用真实 NAS 历史规模测 SQL 写入、事件字节、UI 时延；不先实现分页协议或增量事件 |
| API 契约是否覆盖所有 handler/错误组合 | 历史测试评估已列部分入口缺口；本次没有逐方法生成覆盖率 | 先按风险补 AUD-01 并发与关键删除/代理 HTTP 契约；不能因没有同名 spec 断言功能未测 |
| 薄前端 service 是否值得逐个独立测试 | store、组件与浏览器有间接覆盖，浏览器使用 fixture | 按真实参数/错误映射风险补充；不为每个纯透传函数机械增加镜像测试 |
| 外部依赖最新漏洞、发布 AI 服务和所有外链 | 本次检查用途/边界，未查实时漏洞公告或调用远端发布服务 | 单独执行既有 audit:deps / 发布沙箱联调；不能把无冗余结论写成依赖安全通过 |

固定 2.8.6 的异常 Content-Range 源可令任务保持 active，普通暂停失败。本次引擎专项再次失败；应用层原生安全测试确认暂停/删除被拒绝，强停后保留原 GID 和文件，但强停不代表引擎已提交进度或缺陷已修复。该限制已在未来计划与历史测试评估登记。

已有 `shortcut:` 说明的 DHT 测试仅覆盖出站 IPv4 探测，见 [aria2-fixtures.mjs](../scripts/extended/aria2-fixtures.mjs#L163)；其边界被明确记录，不把它转写成新的生产故障。Linux 中文路径、权限、代理/DNS、设备重启恢复等仍需目标环境验证。

## 5. 最小清理路线

本次只形成以下建议，不实施任何清理。建议按问题类型分小批提交，不建立一个跨领域“全面重构”分支。

| 顺序 | 工作 | 前置条件与边界 | 与现有计划关系 |
| --- | --- | --- | --- |
| 1 | 修复 AUD-01 的共享设置读改写和发布顺序，补确定性交错测试 | 先冻结 Token/普通设置各自失败语义；保持现有接口和 schema | 可纳入 FUTURE-REFACTOR-01 的小范围先行修复；无需等待 FUTURE-KEY-01 |
| 2 | 独立文档修正 AUD-10，明确 AUD-13 的当前规范/设计目标 | 以 architecture/api-contract 为升级事实；视觉数值先确认，不直接改 UI | 与 ENGINE-UPGRADE-290 的升级说明重合 |
| 3 | 独立删除 AUD-09 第三布局、AUD-12 不使用的标签回退；从组合验证移除 AUD-07 重跑阶段 | 再次排除动态入口，迁移测试默认模式；保留两个真实布局、独立 test:filesystem | 不改变公开契约，无数据迁移 |
| 4 | 合并 AUD-11 弹窗及 AUD-12 分类映射 | 保持状态、事件、未知分类与语言切换 | 低风险 UI 维护；无须新建抽象框架 |
| 5 | 合并 AUD-06 基础夹具并加入资源所有权清理；替换 AUD-08 中误绑实现的断言 | 不合并情景专属 mock；保留安全静态守卫，验证并行执行与失败诊断 | 不以削减测试数为验收目标 |
| 6 | 处理 AUD-02 的变化筛选、文件快照重复写；迁移 AUD-05 依赖装配 | 先建立混合负载写计数与操作并发保护；保留事务、重启对账与清理语义 | 与 FUTURE-TASKS-01 的数据安全边界相关，不重做已完成回收站功能 |
| 7 | 完成 AUD-04 的设置 service 编排收口 | 复用第 1 步边界，保存成功/即时应用失败语义不变 | 直接复用 FUTURE-REFACTOR-01 |
| 8 | 决策并逐步收口 AUD-03 的旧引擎执行支持 | 先更新架构支持边界、验证外部路径用户与测试替身；保留旧数据和迁移/回滚 | 属 ENGINE-UPGRADE-290 的兼容收尾；涉及数据的变更须另行评审 |

**可以直接删除的候选**限于确认无生产入口的 UI 分支/回退和组合命令中的重复阶段；要同步其调用与测试，不能机械删文件。**需要合并重构**的是弹窗、分类、夹具、设置编排和装配位置。**涉及兼容/数据的事项**是引擎执行模式，以及若未来选择拆分 Token 存储时的迁移；当前修复 AUD-01 不要求拆表。

有证据的规模估计：第三布局当前为 96 行；详情回退约 20 行；两个约 46 行的确认视图合并后预计可净减约 30–40 行，实际取决于参数与调用调整。这三项量级约为 **140–160 行前端实现**，不把整份双组件或相关测试全部计作收益。日志映射与夹具可进一步减少副本，但本次不报未经实施的精确净行数；旧代引擎潜在删减不计入。**依赖删减 0 个；运行耗时和 NAS 写入收益未量化。**

## 6. 本地验证结果与复现依据

### 6.1 环境与执行方式

| 项目 | 实际环境 |
| --- | --- |
| 平台 | macOS 15.7.5，Darwin / arm64 |
| Node / pnpm | v24.19.0 / 11.17.0 |
| 默认 Rust | Homebrew rustc/cargo 1.93.0；不符合仓库固定版本 |
| 本次标准 Rust | rustc 1.97.1（8bab26f4f），cargo 1.97.1；使用已安装 rustup 工具链 |
| 执行前缀 | 将 rustup 的 bin 目录置于 Homebrew 前，并设置 `RUSTUP_TOOLCHAIN=1.97.1`，避免默认 `rustc/cargo 1.93.0` 被调用 |
| 验证产物 | 日志、Playwright 输出与临时复现程序均放仓库外的系统临时目录；dist/target 是被忽略的本地构建产物 |

完整验证只执行一次，使用上述前缀运行 `pnpm run verify`，总耗时 **188.41s**。1.93.0 只用于识别默认环境差异，没有把它的结果当成标准验证结果。已有依赖和浏览器可用，未为审计升级依赖或改锁文件。

本机原始日志和 settings-race 临时 Cargo 项目保存在系统临时目录，未提交，且可能已由系统清理。报告记录了关键结果、复现交错及候选引擎哈希，不依赖机器专属路径。

### 6.2 逐阶段结果

| 命令/阶段 | 状态 | 实际结果与边界 |
| --- | --- | --- |
| verify：版本与 Rust fmt | 通过 | 版本文件一致为 2.0.0；分支名 2.0.1 不是发布版本已变更的证据 |
| verify：Node 构建/发布脚本 | 通过 | 90 passed，6.48s |
| verify：FPK Shell | 通过 | 进程身份、启动孤儿对账、停止卸载、就绪 4 套模拟测试；不是真实平台生命周期 |
| verify：Rust 测试 | 通过 | 648 passed，1 ignored，9 个测试目标；131.73s；脚本按 warnings as errors 执行 |
| verify：Rust build | 通过 | 固定 1.97.1，warnings as errors |
| verify：Vitest | 通过 | 98 个 spec、477 tests，18.76s |
| verify：类型检查＋生产 Web 构建 | 通过 | vue-tsc 与 Vite，10.91s |
| `rtk proxy pnpm run test:e2e --output=<仓库外目录>` | 通过 | desktop/mobile Chromium 共 18 passed，20.4s；HTTP/SSE 为 fixture，未连接真实 Rust＋引擎 |
| `test:filesystem` 独立阶段 | 复用，未重复执行 | 五组过滤用例已包含于上述完整 Rust 测试，无额外 feature/故障条件；见 AUD-07 |
| `MOTRIX_ARIA2_CANDIDATE_DIR=… pnpm run test:aria2` | **失败且有跳过** | 34 tests：12 passed、1 failed、21 skipped，76.63s；失败为已知异常 Content-Range 暂停失败 |
| 固定候选的 ignored 原生安全测试 | 通过 | 1 passed，13.07s；真实 handler→引擎 RPC→异常 HTTP 源，未走浏览器/TCP 管理 listener |
| AUD-01 仓库外真实 SQLite 并发复现 | **确认缺陷** | 100 次；lost_token=43、lost_setting=55，两个保存调用均返回成功 |
| AUD-08 变量重命名定向检查 | 确认测试脆弱性 | 原源码正则无法识别只改标识符的等价步骤数组 |
| 现有 docs 的本地文件目标检查 | 通过 | 9 份文档未发现缺失本地目标；未验证全部标题锚点和外网链接 |
| FPK 新构建/解包、SBOM、发布 | 未执行 | 本次范围明确排除，不能复用旧产物结果宣称本次交付验证通过 |
| Linux / fnOS / 长期负载 | 未验证 | 本机平台不满足；未操作真实 NAS |
| 实时依赖漏洞审计 | 未执行 | 既有独立每周入口；本次仅查依赖用途和交付边界 |

没有执行一个“全通过的 verify:extended”：实际按阶段运行独立检查，已复用相同用例，且真实引擎专项存在失败。该失败后继续完成了独立原生安全及审计检查，没有自动修复、替换引擎或重跑完整 verify。

### 6.3 固定引擎候选与已知失败

本机已有 macOS arm64 候选；审计时通过其固定 SHA-256 校验：

```text
aria2-next-2.8.6-macos-arm64
SHA256: 6767bc6e79e22dbc491fe7bbcd36daca9dc0fb3b28abca7b9a9cfb8e6e67ece5
```

实际计算的 SHA256 与专项测试固定值相同，因而执行候选专项；没有以任意 PATH 上的 aria2 代替。命令为：

```sh
rtk proxy env MOTRIX_ARIA2_CANDIDATE_DIR=/path/to/candidate-directory pnpm run test:aria2

rtk proxy env PATH="<rustup-bin-directory>:$PATH" RUSTUP_TOOLCHAIN=1.97.1 MOTRIX_ARIA2_CANDIDATE_DIR=/path/to/candidate-directory cargo test --manifest-path server/Cargo.toml --test native_engine_safety -- --ignored --nocapture
```

失败定位：[aria2-sidecar.test.mjs](../scripts/extended/aria2-sidecar.test.mjs#L537) 第 537 行起的“290-14 inconsistent Content-Range … safely stoppable”，第 570 行暂停确认期望 paused、实际 active。21 项跳过包含 Linux 专用运行/权限/代理组合，跳过不记为通过。

原生安全测试输出为 `pause/delete rejected; shutdown 423.756125ms; forced=true; original GID and 130901 file bytes retained`。这证明本次故障下应用的拒绝与文件保全路径通过；不证明强停无进度回退，也不证明 Linux 或 fnOS 同样通过。

### 6.4 AUD-01 最小复现说明

临时 Cargo 项目通过 path 依赖本仓库 server，依赖 tokio，并使用隔离 SQLite 与下载目录；没有给生产代码增加测试导出。核心交错如下，可作为后续确定性回归的起点：

```rust
// 每轮先保存合法初始配置：max_concurrent_downloads = 5，Token = "audit-old-token"。
// next 的默认目录、accessible_paths 和 app_data_dir 使用隔离且合法的目录。
let (settings_result, token_result) = tokio::join!(
    save_app_config(&db.pool, next, &default_dir, &accessible_paths, &app_data_dir),
    save_json_rpc_token(&db.pool, "audit-new-token"),
);
settings_result.unwrap();
token_result.unwrap();
// 再分别 load_json_rpc_token / load_app_config_from_pool，核对新 Token 和并发数 7。
// 本次 100 轮结果：lost_token=43，lost_setting=55。
```

复现直接使用生产保存函数，确认数据库层问题；没有把随机运行的计数作为稳定断言方案。后续正式测试应控制两次读操作与写操作的先后，分别覆盖两个方向的丢失。

### 6.5 与历史测试评估的差异

[testing-evaluation.md](testing-evaluation.md#L5) 的日期为 2026-10-22、基线 42c8eaf，记录 Rust/前端/脚本 617/465/91，浏览器 14；本次 64c861c 为 648/477/90，浏览器 18。不同提交的测试数不能直接解释为覆盖率上升或退化，尤其脚本 91→90 需要查变更才可解释，不能靠数字补一个“丢测试”问题。

历史已记录 macOS 原生组合 12 通过、1 个异常 Content-Range 普通暂停失败及安全处置通过；本次结果与这一限制一致。历史的双架构候选 FPK 检查没有在本次重做；未来计划中 548/461/67 或 471 等数字是各事项当时的验证快照，不作为当前全部测试总数。现有实机、覆盖率报告和长期稳定性缺口仍然保留。

## 7. 审计限制与验收

- 文件清单完整，重点路径深入；其余模块按覆盖表定向检查，**没有逐行阅读全部 588 个文件或每条测试断言**。
- 没有收集 Rust/Vitest 覆盖率，也没有证明全部 API 文档字段、动态组合、诊断脱敏分支和 CSS 呈现均正确。
- 本地浏览器使用模拟服务；真正的 Rust＋引擎测试也不等于浏览器、三类 listener、反向代理和 NAS 的完整贯通。
- 未验证 Linux、fnOS、x86_64 目标运行、真实授权撤销、安装升级/卸载回滚、飞牛 App WebView、FN Connect、长期负载和真实代理网络。
- 本地通过不能替代未来计划的实机门禁；AUD-01 尚未修复，异常源上游缺陷依然存在。
- 交付核验：报告本地文件引用和行号范围有效，附录与基线 588 个跟踪文件一致，空白检查通过；HEAD 未变，Git 状态仅新增本报告，没有既有跟踪文件差异，仓库外验证材料未提交。

结论：先修复共享设置丢失更新，再减少监控重复写，随后按小批次删除闲置 UI 和合并测试/文案副本。保留文件安全、恢复与鉴权边界，先明确旧引擎支持政策再动兼容路径。

## 附录：基线完整文件清单

以下清单由基线 `git ls-files` 生成，覆盖全部 588 个跟踪文件，不包含本次新增报告。**清单纳入不代表逐文件深入阅读**；具体检查深度以第 1.2 节矩阵为准。测试文件的执行通过也不等于每条断言都经过人工审查。



<details>
<summary>.githooks：2 个文件</summary>

```text
.githooks/pre-commit
.githooks/pre-push
```

</details>

<details>
<summary>.github：9 个文件</summary>

```text
.github/ISSUE_TEMPLATE/bug_report.yml
.github/ISSUE_TEMPLATE/config.yml
.github/ISSUE_TEMPLATE/feature_request.yml
.github/workflows/cleanup-actions-caches.yml
.github/workflows/dependency-audit.yml
.github/workflows/prune-actions.yml
.github/workflows/prune-releases.yml
.github/workflows/release.yml
.github/workflows/verify.yml
```

</details>

<details>
<summary>assets：12 个文件</summary>

```text
assets/aria2/aria2-next-2.8.6-checksums.sha256
assets/aria2/aria2-next-2.8.6-dependencies.env
assets/aria2/aria2-next-aarch64-unknown-linux-gnu
assets/aria2/aria2-next-x86_64-unknown-linux-gnu
assets/icons/logo/README.md
assets/icons/logo/fnos.svg
assets/icons/logo/logo-fnos-a-1024.svg
assets/icons/logo/logo-fnos-a-64.svg
assets/icons/logo/logo-fnos-b-1024.svg
assets/icons/logo/logo-fnos-b-64.svg
assets/icons/logo/logo-fnos-c-1024.svg
assets/icons/logo/logo-fnos-c-64.svg
```

</details>

<details>
<summary>docs：9 个文件</summary>

```text
docs/api-contract.md
docs/architecture.md
docs/design/DESIGN.md
docs/design/ui-product-requirements.md
docs/development-scripts.md
docs/fpk-packaging.md
docs/future-development-plan.md
docs/jsonrpc-remote-access.md
docs/testing-evaluation.md
```

</details>

<details>
<summary>e2e：1 个文件</summary>

```text
e2e/browser-smoke.spec.ts
```

</details>

<details>
<summary>packaging：24 个文件</summary>

```text
packaging/fnos/ICON.PNG
packaging/fnos/ICON_256.PNG
packaging/fnos/MotrixFNOS.sc
packaging/fnos/app/ui/config
packaging/fnos/cmd/common.sh
packaging/fnos/cmd/config_callback
packaging/fnos/cmd/config_init
packaging/fnos/cmd/install_callback
packaging/fnos/cmd/install_init
packaging/fnos/cmd/main
packaging/fnos/cmd/reset-web-auth
packaging/fnos/cmd/start
packaging/fnos/cmd/status
packaging/fnos/cmd/stop
packaging/fnos/cmd/uninstall_callback
packaging/fnos/cmd/uninstall_init
packaging/fnos/cmd/upgrade_callback
packaging/fnos/cmd/upgrade_init
packaging/fnos/config/privilege
packaging/fnos/config/resource
packaging/fnos/manifest.template
packaging/fnos/wizard/install
packaging/fnos/wizard/uninstall
packaging/fnos/wizard/upgrade
```

</details>

<details>
<summary>public：1 个文件</summary>

```text
public/icon.png
```

</details>

<details>
<summary>scripts：61 个文件</summary>

```text
scripts/build/build-fpk-all.mjs
scripts/build/build-fpk.mjs
scripts/build/build-server-linux.mjs
scripts/build/build-web-ui-fpk.mjs
scripts/build/build-web.mjs
scripts/build/fetch-aria2-next.mjs
scripts/build/package-local.mjs
scripts/build/package-release.mjs
scripts/build/stage-aria2-sidecar.mjs
scripts/extended/aria2-fixtures.mjs
scripts/extended/aria2-sidecar.test.mjs
scripts/lib/command-progress.mjs
scripts/lib/script-utils.mjs
scripts/maintenance/clean-generated.mjs
scripts/maintenance/clean-rust-target.mjs
scripts/release/generate-fpk-sbom.mjs
scripts/release/release-changelog-ai.mjs
scripts/release/release-changelog-cloudflare.mjs
scripts/release/release-changelog-context.mjs
scripts/release/release-notes.mjs
scripts/release/release-prepare.mjs
scripts/tests/actions-pinning.test.mjs
scripts/tests/aria2-assets.test.mjs
scripts/tests/aria2-fixtures.test.mjs
scripts/tests/bootstrap-shell.test.mjs
scripts/tests/command-progress.test.mjs
scripts/tests/dependency-audit.test.mjs
scripts/tests/fpk-artifact.test.mjs
scripts/tests/fpk-identity.test.mjs
scripts/tests/fpk-upgrade-lifecycle.test.mjs
scripts/tests/generate-fpk-sbom.test.mjs
scripts/tests/logging-docs.test.mjs
scripts/tests/release-changelog-cloudflare.test.mjs
scripts/tests/release-changelog-context.test.mjs
scripts/tests/release-notes.test.mjs
scripts/tests/release-prepare-fallback.test.mjs
scripts/tests/release-retention.test.mjs
scripts/tests/run-rust-tests.test.mjs
scripts/tests/rust-cache-policy.test.mjs
scripts/tests/script-utils.test.mjs
scripts/tests/style-architecture.test.mjs
scripts/tests/test-code-separation.test.mjs
scripts/tests/test-duration-reporter.test.mjs
scripts/tests/theme-architecture.test.mjs
scripts/tests/toolchain.test.mjs
scripts/tests/vitest-progress-reporter.test.mjs
scripts/verify/dependency-audit.mjs
scripts/verify/run-filesystem-tests.mjs
scripts/verify/run-rust-tests.mjs
scripts/verify/test-duration-reporter.mjs
scripts/verify/test-fnos-lifecycle-stop.sh
scripts/verify/test-fnos-process-identity.sh
scripts/verify/test-fnos-readiness.sh
scripts/verify/test-fnos-startup-reconcile.sh
scripts/verify/verify-extended.mjs
scripts/verify/verify-fpk-artifacts.mjs
scripts/verify/verify.mjs
scripts/verify/vitest-progress-reporter.mjs
scripts/version/version-check.mjs
scripts/version/version-set.mjs
scripts/version/version-utils.mjs
```

</details>

<details>
<summary>server：174 个文件</summary>

```text
server/Cargo.lock
server/Cargo.toml
server/src/api/app.rs
server/src/api/app/tests.rs
server/src/api/aria2.rs
server/src/api/auth.rs
server/src/api/auth/tests.rs
server/src/api/debug_logs.rs
server/src/api/diagnostics.rs
server/src/api/diagnostics/tests.rs
server/src/api/error.rs
server/src/api/events.rs
server/src/api/events/tests.rs
server/src/api/extract.rs
server/src/api/jsonrpc/add_uri.rs
server/src/api/jsonrpc/auth.rs
server/src/api/jsonrpc/methods.rs
server/src/api/jsonrpc/mod.rs
server/src/api/jsonrpc/tests.rs
server/src/api/jsonrpc/types.rs
server/src/api/mod.rs
server/src/api/settings.rs
server/src/api/settings/tests.rs
server/src/api/storage.rs
server/src/api/storage/tests.rs
server/src/api/tasks.rs
server/src/api/tasks/context.rs
server/src/api/tasks/request.rs
server/src/api/tasks/tests.rs
server/src/api/tests.rs
server/src/app/compatibility.rs
server/src/app/mod.rs
server/src/app/tests.rs
server/src/aria2/args.rs
server/src/aria2/config_status.rs
server/src/aria2/log_mode.rs
server/src/aria2/log_mode/tests.rs
server/src/aria2/logging.rs
server/src/aria2/mod.rs
server/src/aria2/options.rs
server/src/aria2/options/tests.rs
server/src/aria2/ports.rs
server/src/aria2/process_probe.rs
server/src/aria2/rpc.rs
server/src/aria2/rpc/tests.rs
server/src/aria2/runtime_file.rs
server/src/aria2/session.rs
server/src/aria2/session/tests.rs
server/src/aria2/sidecar.rs
server/src/aria2/tests.rs
server/src/auth/jwt.rs
server/src/auth/jwt/tests.rs
server/src/auth/mod.rs
server/src/auth/password.rs
server/src/auth/password/tests.rs
server/src/auth/process_lock.rs
server/src/auth/rate_limit.rs
server/src/auth/rate_limit/tests.rs
server/src/auth/tests.rs
server/src/config/aria2.rs
server/src/config/mod.rs
server/src/database/engine_state.rs
server/src/database/engine_state/tests.rs
server/src/database/maintenance.rs
server/src/database/maintenance/tests.rs
server/src/database/mod.rs
server/src/database/settings.rs
server/src/database/settings/tests.rs
server/src/database/task_operations.rs
server/src/database/task_operations/tests.rs
server/src/database/tasks.rs
server/src/database/tasks/tests.rs
server/src/database/tests.rs
server/src/database/web_auth.rs
server/src/debug_logs/mod.rs
server/src/debug_logs/tests.rs
server/src/diagnostics/bundle.rs
server/src/diagnostics/bundle/tests.rs
server/src/diagnostics/mod.rs
server/src/diagnostics/storage.rs
server/src/diagnostics/storage/tests.rs
server/src/fnos/mod.rs
server/src/fnos/tests.rs
server/src/lib.rs
server/src/main.rs
server/src/runtime/aria2_log_maintenance.rs
server/src/runtime/aria2_log_maintenance/tests.rs
server/src/runtime/aria2_log_mode.rs
server/src/runtime/aria2_process/mod.rs
server/src/runtime/aria2_process/resolve.rs
server/src/runtime/aria2_process/start.rs
server/src/runtime/aria2_process/start/startup_config.rs
server/src/runtime/aria2_process/start/startup_config/tests.rs
server/src/runtime/aria2_process/status.rs
server/src/runtime/aria2_process/stop.rs
server/src/runtime/aria2_process/stop/tests.rs
server/src/runtime/aria2_process/tests.rs
server/src/runtime/aria2_process/types.rs
server/src/runtime/file_cleanup.rs
server/src/runtime/file_cleanup/tests.rs
server/src/runtime/lifecycle.rs
server/src/runtime/lifecycle/tests.rs
server/src/runtime/mod.rs
server/src/runtime/shutdown.rs
server/src/runtime/shutdown/tests.rs
server/src/runtime/task_monitor.rs
server/src/runtime/task_monitor/tests.rs
server/src/runtime/task_operation_reconcile.rs
server/src/runtime/task_operation_reconcile/tests.rs
server/src/settings/mod.rs
server/src/settings/proxy.rs
server/src/settings/proxy/tests.rs
server/src/settings/service.rs
server/src/settings/service/tests.rs
server/src/state/mod.rs
server/src/state/tests.rs
server/src/storage/display_paths.rs
server/src/storage/display_paths/tests.rs
server/src/storage/mod.rs
server/src/storage/shared_access.rs
server/src/storage/shared_access/tests.rs
server/src/storage/tests.rs
server/src/tasks/aria2_rpc/control.rs
server/src/tasks/aria2_rpc/control/tests.rs
server/src/tasks/aria2_rpc/create.rs
server/src/tasks/aria2_rpc/create/tests.rs
server/src/tasks/aria2_rpc/mod.rs
server/src/tasks/aria2_rpc/query.rs
server/src/tasks/aria2_rpc/query/tests.rs
server/src/tasks/aria2_rpc/transport.rs
server/src/tasks/file_context.rs
server/src/tasks/file_context/tests.rs
server/src/tasks/files.rs
server/src/tasks/files/tests.rs
server/src/tasks/magnet_refresh.rs
server/src/tasks/mod.rs
server/src/tasks/model.rs
server/src/tasks/operation.rs
server/src/tasks/options.rs
server/src/tasks/options/tests.rs
server/src/tasks/prepare.rs
server/src/tasks/progress.rs
server/src/tasks/public.rs
server/src/tasks/refresh.rs
server/src/tasks/repository.rs
server/src/tasks/service.rs
server/src/tasks/service/control.rs
server/src/tasks/service/create.rs
server/src/tasks/service/delete.rs
server/src/tasks/service/magnet.rs
server/src/tasks/service/native_metadata.rs
server/src/tasks/service/native_metadata/bencode.rs
server/src/tasks/service/native_metadata/layers.rs
server/src/tasks/service/native_metadata/tests.rs
server/src/tasks/service/proxy.rs
server/src/tasks/service/query.rs
server/src/tasks/service/query/tests.rs
server/src/tasks/service/restore.rs
server/src/tasks/service/tests.rs
server/src/tasks/session.rs
server/src/tasks/session/native.rs
server/src/tasks/session/tests.rs
server/src/tasks/state.rs
server/src/tasks/status.rs
server/src/tasks/tests.rs
server/src/test_support.rs
server/src/tests.rs
server/tests/database_cli.rs
server/tests/engine_upgrade.rs
server/tests/idle_file_stability.rs
server/tests/native_engine_safety.rs
server/tests/runtime_stability.rs
server/tests/runtime_stability/resources.rs
server/tests/web_auth_cli.rs
```

</details>

<details>
<summary>src：278 个文件</summary>

```text
src/App.spec.ts
src/App.vue
src/app/bootstrap.spec.ts
src/app/bootstrap.ts
src/app/composables/useMobileLayout.spec.ts
src/app/composables/useMobileLayout.ts
src/app/hostPlatform.spec.ts
src/app/hostPlatform.ts
src/app/providers/NaiveProvider.vue
src/app/theme.ts
src/app/utils/clipboard.spec.ts
src/app/utils/clipboard.ts
src/app/utils/errors.spec.ts
src/app/utils/errors.ts
src/components/AppIcon.css
src/components/AppIcon.spec.ts
src/components/AppIcon.vue
src/components/EngineStatusPanel.css
src/components/EngineStatusPanel.spec.ts
src/components/EngineStatusPanel.vue
src/components/ui/AppConfirmDialog.css
src/components/ui/AppConfirmDialog.spec.ts
src/components/ui/AppConfirmDialog.vue
src/components/ui/AppDialog.css
src/components/ui/AppDialog.spec.ts
src/components/ui/AppDialog.vue
src/components/ui/AppDialogActions.css
src/components/ui/AppDialogActions.spec.ts
src/components/ui/AppDialogActions.vue
src/components/ui/AppMetricCard.css
src/components/ui/AppMetricCard.spec.ts
src/components/ui/AppMetricCard.vue
src/components/ui/AppMetricGrid.css
src/components/ui/AppMetricGrid.spec.ts
src/components/ui/AppMetricGrid.vue
src/components/ui/AppSectionCard.css
src/components/ui/AppSectionCard.spec.ts
src/components/ui/AppSectionCard.vue
src/components/ui/appMetric.ts
src/features/about/components/AboutDialog.css
src/features/about/components/AboutDialog.spec.ts
src/features/about/components/AboutDialog.vue
src/features/about/composables/useUpdateCheck.spec.ts
src/features/about/composables/useUpdateCheck.ts
src/features/about/services/aboutService.ts
src/features/about/services/changelogService.spec.ts
src/features/about/services/changelogService.ts
src/features/about/types.ts
src/features/auth/components/AuthGate.css
src/features/auth/components/AuthGate.spec.ts
src/features/auth/components/AuthGate.vue
src/features/auth/components/WebAuthSettings.css
src/features/auth/components/WebAuthSettings.spec.ts
src/features/auth/components/WebAuthSettings.vue
src/features/auth/services/authChannel.spec.ts
src/features/auth/services/authChannel.ts
src/features/auth/services/authService.spec.ts
src/features/auth/services/authService.ts
src/features/auth/stores/authStore.spec.ts
src/features/auth/stores/authStore.ts
src/features/auth/types.ts
src/features/diagnostics/components/Aria2LogModePanel.css
src/features/diagnostics/components/Aria2LogModePanel.spec.ts
src/features/diagnostics/components/Aria2LogModePanel.vue
src/features/diagnostics/components/DebugLogDialog.css
src/features/diagnostics/components/DebugLogDialog.spec.ts
src/features/diagnostics/components/DebugLogDialog.vue
src/features/diagnostics/components/DebugLogList.css
src/features/diagnostics/components/DebugLogList.spec.ts
src/features/diagnostics/components/DebugLogList.vue
src/features/diagnostics/components/DebugLogManualCopyDialog.css
src/features/diagnostics/components/DebugLogManualCopyDialog.spec.ts
src/features/diagnostics/components/DebugLogManualCopyDialog.vue
src/features/diagnostics/components/DiagnosticsDialog.css
src/features/diagnostics/components/DiagnosticsDialog.spec.ts
src/features/diagnostics/components/DiagnosticsDialog.vue
src/features/diagnostics/components/LogMaintenancePanel.css
src/features/diagnostics/components/LogMaintenancePanel.spec.ts
src/features/diagnostics/components/LogMaintenancePanel.vue
src/features/diagnostics/composables/useAria2Status.spec.ts
src/features/diagnostics/composables/useAria2Status.ts
src/features/diagnostics/composables/useDebugLogExport.spec.ts
src/features/diagnostics/composables/useDebugLogExport.ts
src/features/diagnostics/composables/useDebugLogFilters.spec.ts
src/features/diagnostics/composables/useDebugLogFilters.ts
src/features/diagnostics/composables/useDiagnosticBundleExport.spec.ts
src/features/diagnostics/composables/useDiagnosticBundleExport.ts
src/features/diagnostics/services/aria2LogModeService.spec.ts
src/features/diagnostics/services/aria2LogModeService.ts
src/features/diagnostics/services/debugLogService.ts
src/features/diagnostics/services/diagnosticBundleService.spec.ts
src/features/diagnostics/services/diagnosticBundleService.ts
src/features/diagnostics/services/logMaintenanceService.spec.ts
src/features/diagnostics/services/logMaintenanceService.ts
src/features/diagnostics/services/storageDiagnosticsService.spec.ts
src/features/diagnostics/services/storageDiagnosticsService.ts
src/features/diagnostics/stores/debugLogStore.spec.ts
src/features/diagnostics/stores/debugLogStore.ts
src/features/diagnostics/types.ts
src/features/extensions/components/ExtensionsPlaceholder.css
src/features/extensions/components/ExtensionsPlaceholder.vue
src/features/help/components/HelpDialog.css
src/features/help/components/HelpDialog.spec.ts
src/features/help/components/HelpDialog.vue
src/features/settings/components/JsonRpcGuide.css
src/features/settings/components/JsonRpcGuide.vue
src/features/settings/components/JsonRpcGuideDialog.spec.ts
src/features/settings/components/JsonRpcGuideDialog.vue
src/features/settings/components/JsonRpcTokenSettings.css
src/features/settings/components/JsonRpcTokenSettings.spec.ts
src/features/settings/components/JsonRpcTokenSettings.vue
src/features/settings/components/LanJsonRpcSettings.css
src/features/settings/components/LanJsonRpcSettings.spec.ts
src/features/settings/components/LanJsonRpcSettings.vue
src/features/settings/components/ProxySettings.css
src/features/settings/components/ProxySettings.spec.ts
src/features/settings/components/ProxySettings.vue
src/features/settings/components/SettingsDialog.css
src/features/settings/components/SettingsDialog.spec.ts
src/features/settings/components/SettingsDialog.vue
src/features/settings/services/downloadProxyService.spec.ts
src/features/settings/services/downloadProxyService.ts
src/features/settings/services/jsonRpcTokenService.spec.ts
src/features/settings/services/jsonRpcTokenService.ts
src/features/settings/services/lanJsonRpcService.spec.ts
src/features/settings/services/lanJsonRpcService.ts
src/features/settings/stores/downloadProxyStore.spec.ts
src/features/settings/stores/downloadProxyStore.ts
src/features/settings/stores/jsonRpcTokenStore.spec.ts
src/features/settings/stores/jsonRpcTokenStore.ts
src/features/settings/stores/lanJsonRpcStore.spec.ts
src/features/settings/stores/lanJsonRpcStore.ts
src/features/settings/stores/settingsStore.spec.ts
src/features/settings/stores/settingsStore.ts
src/features/settings/utils/lanJsonRpcEndpoint.spec.ts
src/features/settings/utils/lanJsonRpcEndpoint.ts
src/features/tasks/components/TaskActions.css
src/features/tasks/components/TaskActions.spec.ts
src/features/tasks/components/TaskActions.vue
src/features/tasks/components/TaskActionsContainer.vue
src/features/tasks/components/TaskBulkDeleteConfirmDialog.css
src/features/tasks/components/TaskBulkDeleteConfirmDialog.spec.ts
src/features/tasks/components/TaskBulkDeleteConfirmDialog.vue
src/features/tasks/components/TaskCardHeader.css
src/features/tasks/components/TaskCardHeader.spec.ts
src/features/tasks/components/TaskCardHeader.vue
src/features/tasks/components/TaskCompatibilityNotice.vue
src/features/tasks/components/TaskCreateDialog.css
src/features/tasks/components/TaskCreateDialog.spec.ts
src/features/tasks/components/TaskCreateDialog.vue
src/features/tasks/components/TaskDeleteConfirmDialog.spec.ts
src/features/tasks/components/TaskDeleteConfirmDialog.vue
src/features/tasks/components/TaskDesktopCard.css
src/features/tasks/components/TaskDesktopCard.spec.ts
src/features/tasks/components/TaskDesktopCard.vue
src/features/tasks/components/TaskDesktopList.css
src/features/tasks/components/TaskDesktopList.spec.ts
src/features/tasks/components/TaskDesktopList.vue
src/features/tasks/components/TaskDetailsDialog.css
src/features/tasks/components/TaskDetailsDialog.spec.ts
src/features/tasks/components/TaskDetailsDialog.vue
src/features/tasks/components/TaskEmptyState.css
src/features/tasks/components/TaskEmptyState.spec.ts
src/features/tasks/components/TaskEmptyState.vue
src/features/tasks/components/TaskErrorMessage.css
src/features/tasks/components/TaskErrorMessage.spec.ts
src/features/tasks/components/TaskErrorMessage.vue
src/features/tasks/components/TaskFileConfirmCoordinator.vue
src/features/tasks/components/TaskFileConfirmDialog.css
src/features/tasks/components/TaskFileConfirmDialog.spec.ts
src/features/tasks/components/TaskFileConfirmDialog.vue
src/features/tasks/components/TaskMetaItems.css
src/features/tasks/components/TaskMetaItems.spec.ts
src/features/tasks/components/TaskMetaItems.vue
src/features/tasks/components/TaskMobileList.css
src/features/tasks/components/TaskMobileList.spec.ts
src/features/tasks/components/TaskMobileList.vue
src/features/tasks/components/TaskPermanentDeleteConfirmDialog.spec.ts
src/features/tasks/components/TaskPermanentDeleteConfirmDialog.vue
src/features/tasks/components/TaskProgressBar.css
src/features/tasks/components/TaskProgressBar.spec.ts
src/features/tasks/components/TaskProgressBar.vue
src/features/tasks/components/TaskProgressCell.css
src/features/tasks/components/TaskProgressCell.spec.ts
src/features/tasks/components/TaskProgressCell.vue
src/features/tasks/components/TaskProxyToggle.css
src/features/tasks/components/TaskProxyToggle.spec.ts
src/features/tasks/components/TaskProxyToggle.vue
src/features/tasks/components/TaskRedownloadConfirmDialog.spec.ts
src/features/tasks/components/TaskRedownloadConfirmDialog.vue
src/features/tasks/components/TaskRestoreConfirmDialog.spec.ts
src/features/tasks/components/TaskRestoreConfirmDialog.vue
src/features/tasks/components/TaskSourceIcon.vue
src/features/tasks/components/TaskStatusBadge.vue
src/features/tasks/components/TaskTable.css
src/features/tasks/components/TaskTable.spec.ts
src/features/tasks/components/TaskTable.vue
src/features/tasks/components/taskActionTestFixtures.ts
src/features/tasks/components/taskActionViewModel.ts
src/features/tasks/composables/taskCreateFormModel.spec.ts
src/features/tasks/composables/taskCreateFormModel.ts
src/features/tasks/composables/useTaskBulkActions.spec.ts
src/features/tasks/composables/useTaskBulkActions.ts
src/features/tasks/composables/useTaskCategoryView.spec.ts
src/features/tasks/composables/useTaskCategoryView.ts
src/features/tasks/composables/useTaskCreateForm.spec.ts
src/features/tasks/composables/useTaskCreateForm.ts
src/features/tasks/composables/useTaskCreateSubmission.spec.ts
src/features/tasks/composables/useTaskCreateSubmission.ts
src/features/tasks/composables/useTaskCreateValidation.spec.ts
src/features/tasks/composables/useTaskCreateValidation.ts
src/features/tasks/composables/useTaskPagination.spec.ts
src/features/tasks/composables/useTaskPagination.ts
src/features/tasks/composables/useTaskSaveDirectory.spec.ts
src/features/tasks/composables/useTaskSaveDirectory.ts
src/features/tasks/composables/useTaskStatusActions.spec.ts
src/features/tasks/composables/useTaskStatusActions.ts
src/features/tasks/composables/useTaskToasts.spec.ts
src/features/tasks/composables/useTaskToasts.ts
src/features/tasks/composables/useTaskToolbar.spec.ts
src/features/tasks/composables/useTaskToolbar.ts
src/features/tasks/composables/useTaskTopbarActions.spec.ts
src/features/tasks/composables/useTaskTopbarActions.ts
src/features/tasks/services/taskService.spec.ts
src/features/tasks/services/taskService.ts
src/features/tasks/stores/taskStore.spec.ts
src/features/tasks/stores/taskStore.ts
src/features/tasks/utils/taskFormat.spec.ts
src/features/tasks/utils/taskFormat.ts
src/i18n/index.spec.ts
src/i18n/index.ts
src/i18n/locales/en-US.ts
src/i18n/locales/zh-CN.ts
src/layouts/AppShell.css
src/layouts/AppShell.vue
src/layouts/SidebarNav.css
src/layouts/SidebarNav.spec.ts
src/layouts/SidebarNav.vue
src/layouts/Topbar.css
src/layouts/Topbar.spec.ts
src/layouts/Topbar.vue
src/layouts/navigation.ts
src/main.ts
src/services/aria2.ts
src/services/backend.ts
src/services/fnos.spec.ts
src/services/fnos.ts
src/services/http.spec.ts
src/services/http.ts
src/services/runtimeEvents.spec.ts
src/services/runtimeEvents.ts
src/services/settings.spec.ts
src/services/settings.ts
src/services/storage.ts
src/styles/base.css
src/styles/dialog.css
src/styles/mobile-baseline.css
src/styles/tokens.css
src/test/mount.spec.ts
src/test/mount.ts
src/test/setup.spec.ts
src/test/setup.ts
src/types/app.ts
src/types/aria2.ts
src/types/navigation.ts
src/types/settings.ts
src/types/storage.ts
src/types/tasks.ts
src/types/topbar.ts
src/views/MainWindow.css
src/views/MainWindow.spec.ts
src/views/MainWindow.vue
src/views/MainWindowDialogs.vue
src/views/composables/useMainWindowDialogs.spec.ts
src/views/composables/useMainWindowDialogs.ts
src/views/composables/useMainWindowLifecycle.spec.ts
src/views/composables/useMainWindowLifecycle.ts
src/vite-env.d.ts
```

</details>

<details>
<summary>根目录：17 个文件</summary>

```text
.gitignore
.node-version
AGENTS.md
CHANGELOG.md
README.md
SECURITY.md
index.html
package.json
playwright.config.ts
pnpm-lock.yaml
pnpm-workspace.yaml
rust-toolchain.toml
tsconfig.json
tsconfig.node.json
uno.config.ts
vite.config.ts
vitest.config.ts
```

</details>


## 7. 清理实施记录（2026-10-26 起）

本节记录原审计后的授权修复，不改变第 1–6 节的历史测试基线。四阶段依次验证、打包并提交；最终运行支持边界已确定为仅 Aria2 Next 2.8.6，旧代数据继续保留。

### 阶段 1：配置并发与启动门禁

- 普通设置和公网 Token 的读改写使用同一 SQLite `BEGIN IMMEDIATE` 事务；生产入口在 settings 服务内串行完成保存、缓存发布和即时应用。JSON 格式和公开接口不变。
- 移除缺失引擎回退到 LegacyV1 的行为，未确认身份前默认受限。受限启动不加载任务、不执行任务或文件恢复。
- 补充直接持久化并发、缓存发布顺序、失败不发布、重新加载，以及缺失/未知/不可执行/超时引擎的数据保全回归。
- 原先依靠“本机没有引擎也能下载”回退的测试夹具改为明确提供引擎身份，没有放松生产门禁。
- 执行基线：`feature/2.0.1`、`64c861cd85f31f983438ed9252054299f351b80c`，初始工作区仅本报告未跟踪。使用 Rust 1.97.1（默认 1.93.0 未作为标准结果）、Node 24.19.0、pnpm 11.17.0。
- 定向回归：文件清理 7/7、设置 API 16/16 通过；标准 `pnpm run build:fpk` 最终通过，Rust 651 passed / 1 ignored / 9 目标、前端 477/477（98 文件）、脚本 90/90、4 组 Shell、类型检查、生产构建、x86/ARM FPK 及解包验收通过，232.68 秒。首轮失败为尚未显式提供引擎身份的空闲稳定性夹具；补齐后回归通过。文件清理夹具的旧临时数据库重名也已按进程隔离。
- 业务代码新增 166 / 删除 111 / 净增 55 行；测试新增 291 / 删除 19 / 净增 272 行（含测试工具）。开发计划新增 2 / 删除 2；本报告为首次入库文档，单独计入，不作为代码膨胀或删减收益。

| 阶段 1 产物（字节） | ARM64 | x86_64 |
| --- | ---: | ---: |
| FPK | 16,139,166 | 16,071,053 |
| 解包后端 | 14,725,304 | 16,399,200 |
| 解包前端 | 1,278,692 | 1,278,692 |
| 解包引擎 | 18,317,056 | 19,252,656 |
| FPK SHA-256 | `8b1e6ddb176c4d3ca6b3cc6247873d56f8089c5d0ea57ebcbdd9cbde2d27663c` | `b9a77f42e9965fad36bec0051ded30459a9fc9c3ebc9bfb3d25c01967f9a67f5` |

产物、日志与机器可读体积记录归档在仓库外 `/var/folders/f4/z5176rn52p7cv3wrh6xhsr5m0000gn/T/motrix-cleanup-20261026-jneu2pam/phase1/`。原始包基线存于同级 `baseline/`；基线 ARM/x86 FPK 分别为 16,149,667 / 16,071,076 字节，后端 14,734,512 / 16,395,088 字节，前端与引擎均未变化。基线包与本地源码产物匹配，但未内嵌提交号，不能把它当成带构建证明的发布对照。

本阶段未运行追加 E2E 或原生引擎专项；按照计划在第 3、4 阶段执行。未操作 NAS，Linux 运行行为、fnOS 实机及长期负载仍未验证。


### 阶段 2：AUD-02 写入放大

`runtime/task_monitor.rs` 不再全量保存同代任务。`tasks/refresh.rs` 比较完整 Rust 结构的持久化内容（含私密代理绑定，排除仅轮询时间变化），复用任务操作保护，同时核对内存原快照及 SQLite 原快照；数据库成功提交后再发布到内存。完成状态保存失败时仍保持原活动快照，下轮可重试；完成控制文件也仅在提交且内存条件发布成功后清理。回收站的待确认磁链不参与刷新，退出的最终持久化入口继续保留。

`database/tasks.rs` 按索引同步文件：相同文件不发写入，变化项更新、新项插入、消失项删除。原路径验证、重复索引拒绝、代际校验及事务回滚保留，文件核对时间只随真实内容变化更新。未引入依赖、迁移、生产计数器、刷新节流或 SSE 协议变更。

**测量方法与边界：** 对比阶段 1 提交 `cc88563` 和本阶段实现，使用仓库外 Rust 驱动直接调用真实 `monitor_tasks_once`；同一 macOS ARM64 本地磁盘 SQLite，WAL / synchronous=NORMAL，调试构建配置一致。可控回环 RPC 每轮仅让活动 BT 任务的第 1 个文件增加 17 字节，其余 99 个文件不变。两个场景分别为 1 个活动 BT（100 文件），以及另加 300 个历史 BT（各 100 文件，完成/暂停/回收站各 100 个）。每组预热 2 轮后测 10 轮，重复 3 次。写入计数通过测试数据库的 AFTER 触发器获得，耗时在无触发器的独立数据库测量；预热和建库均不计入。计时期间没有同时执行完整测试或交叉编译；早期调试数据不纳入以下结果。

| 每次 10 轮的行写入（3 次结果一致） | 修改前 | 修改后 |
| --- | ---: | ---: |
| 活动任务 UPDATE | 10 | 10 |
| 活动文件 DELETE / INSERT / UPDATE | 1,000 / 1,000 / 0 | 0 / 0 / 10 |
| 300 个历史任务 UPDATE | 3,000 | 0 |
| 历史文件 DELETE / INSERT / UPDATE | 300,000 / 300,000 / 0 | 0 / 0 / 0 |

| 无计数插桩，每轮毫秒；3 次 × 10 轮 | 修改前中位数（最小–最大） | 修改后中位数（最小–最大） |
| --- | ---: | ---: |
| 单活动任务，合并 30 样本 | 4.550（3.893–6.332） | 4.375（3.654–6.135） |
| 单活动 + 300 历史，合并 30 样本 | 622.912（542.898–808.900） | 25.814（23.580–28.424） |
| 单活动任务，第 1 / 2 / 3 次中位数 | 4.371 / 4.749 / 4.658 | 4.535 / 3.961 / 4.543 |
| 300 历史，第 1 / 2 / 3 次中位数 | 627.511 / 622.912 / 619.922 | 26.210 / 25.499 / 25.761 |

单活动任务没有测得耗时收益，小幅差异处于重叠范围；不能把减少写入直接描述为该场景提速。历史任务场景的收益来自避免重写历史及不变文件，不代表下载吞吐、NAS CPU 或 Linux 磁盘性能。测量驱动、方法与逐轮 JSON 在归档根目录的 `aud02-bench/`、`aud02-baseline/`、`aud02-final-safe/`，未提交临时验证工程。

**阶段验证：** 刷新并发/失败/控制文件保全/回收站保护 4/4、数据库任务专项 18/18；最终标准 `pnpm run build:fpk` 通过，Rust 656 passed / 1 ignored / 9 目标、前端 477/477（98 文件）、脚本 90/90、4 组 Shell、类型检查、生产构建及双架构验包通过，222.96 秒。中途复核新增边界保护后中止了未完成的构建，以上仅引用最终完整入口，不把早期中止结果算作通过。

业务新增 176 / 删除 83 / 净增 93 行；测试新增 372 / 删除 0 / 净增 372 行；文档新增 39 / 删除 0。这里是以少量代码和条件读校验换取历史场景的写入与耗时下降，不属于代码或包体删减阶段。没有测量真实下载吞吐。

| 阶段 2 产物（字节） | ARM64 | x86_64 |
| --- | ---: | ---: |
| FPK | 16,188,673 | 16,155,608 |
| 解包后端 | 14,835,816 | 16,548,976 |
| 解包前端 | 1,278,692 | 1,278,692 |
| 解包引擎 | 18,317,056 | 19,252,656 |
| FPK SHA-256 | `5f7069bbadb794360cc82f1681216fa82feeaad1a49b36fc81fb205dea8101f7` | `51da79fe0ac448d4e54fc1c2dd10b73e96240c00cb455e9447172aea73d5152c` |

产物及最终日志在归档根目录的 `phase2/`。相对阶段 1，FPK 增加 49,507 / 84,555 字节，后端增加 110,512 / 149,776 字节；引擎、前端未变化。受支持 Linux/fnOS 的运行、长期负载、实际 BT 网络和下载吞吐仍未验证。
