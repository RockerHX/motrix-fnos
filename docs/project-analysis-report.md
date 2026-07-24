# Motrix fnOS 项目分析报告

> 报告类型：当前代码基线的精简决策版
> 复核日期：2026-07-24
> 分析基线：`35b3a8e`
> 项目版本：`1.8.1`
> 分析范围：`server/`、`src/`、`packaging/fnos/`、`scripts/`、`.github/workflows/` 与关键维护文档
> 分析方式：静态代码审阅、配置和脚本检查、现有自动化测试；fnOS 实机项目另列为待完成证据

## 1. 结论摘要

Motrix fnOS 当前采用单 FPK、单 Rust server、Aria2 sidecar、SQLite 和 Vue Web UI 的架构。管理面与 JSON-RPC 面分开监听，长期数据由 SQLite 和 Aria2 session 保存，前端通过 HTTP API 和 SSE 工作。当前没有需要更换主技术栈或拆成微服务的问题。

当前决策结论：

| 优先级 | 结论 | 下一步 |
| --- | --- | --- |
| P0 | 重新下载已不再先删除用户文件；URL、种子和磁链按来源创建新任务，失败会回滚 | 保留现有回滚测试，发布前补真实设备证据 |
| P1 | 操作记录、迁移、任务互斥、补偿、RPC 超时、资源限制、readiness、SSE 重同步和前端竞态已在代码中完成 | 需要进程崩溃、断电、磁盘异常和网络半断等外部故障证据 |
| P2 | 数据库维护、登录部署规则、日志治理、CI/供应链和 FPK 预检已完成；前端安全补丁没有明确收益，未强行升级 | fnOS 安装生命周期、代理链路和 UI 设计批准仍是发布边界 |

本报告把四类状态用直白的话区分：外部下载引擎、运行时内存、持久数据库、用户文件。SQLite 事务只能保护数据库；跨越 Aria2 和文件系统的动作依靠持久操作记录、补偿和启动对账收敛，不能声称是一个跨系统事务。

## 2. 验证基线

### 2.1 本地验证结果

以下结果针对分析基线重新执行；测试数量是当前代码的数量，不引用历史版本数字。

| 命令 | 结果 |
| --- | --- |
| `pnpm run version:check` | 通过，版本源统一为 `1.8.1` |
| `pnpm run test:scripts` | 35 项通过 |
| `RUSTFLAGS='-D warnings' cargo test --manifest-path server/Cargo.toml` | 278 项通过（Rust 库 275 项，数据库 CLI 集成 3 项） |
| `pnpm run typecheck` | 通过 |
| `pnpm run test:unit` | 80 个测试文件、309 项通过 |
| `pnpm run verify:pre-commit` | 通过，包含上面的快速验证和 FPK 脚本检查 |
| `pnpm run audit:deps` | 通过；`cargo-audit 0.22.2` 与 `pnpm audit --prod` 均未发现高危或严重漏洞 |
| `pnpm run verify` | 通过，包含 Rust 编译和 Web UI 生产构建 |
| `pnpm run build:fpk:prepare` | 通过，x86_64 与 aarch64 双架构 stage 和预检完成，未调用 fnpack |
| `git diff --check` | 通过 |

这两项只能证明本地构建和双架构预组装，不等同于 fnOS 实机通过。

### 2.2 尚未执行的验证

本轮没有真实执行 fnOS 安装、同身份升级、停止/重启/卸载/回滚、FN Connect 远程访问、Lucky 或其他反向代理、真实 SSE/WebSocket 代理链路，也没有做断电、磁盘满、SQLite 只读或 Aria2 半断连接故障注入。相关结论保持“待实机验证”，不能写成发布通过。

证据模板：

- `docs/fnos-install-upgrade-acceptance.md`
- `docs/fnos-lifecycle-acceptance.md`
- `docs/fnos-connect-proxy-acceptance.md`

## 3. 当前架构

### 3.1 运行拓扑

```text
fnOS FPK
  ├─ manifest / 权限 / 生命周期脚本 / Web 入口
  ├─ Rust + Axum server
  │   ├─ 管理 listener（默认 0.0.0.0:17080）
  │   │   └─ Web UI、HTTP API、SSE
  │   ├─ JSON-RPC listener（默认 127.0.0.1:17081）
  │   │   └─ JSON-RPC HTTP、WebSocket、CORS 预检
  │   ├─ SQLite
  │   └─ Aria2 Next sidecar
  └─ Vue 3 + Pinia + Naive UI
```

两个 listener 共享任务、数据库、Aria2 运行态和退出信号，但对外边界不同：管理端口服务 Web UI 和管理 API，JSON-RPC 端口只在回环地址监听，外部代理必须明确指向它。

### 3.2 主要数据流

```text
Vue 组件
  -> Pinia store
  -> feature service
  -> 管理 HTTP API / SSE
  -> Rust service
  -> Aria2 RPC、SQLite、用户文件
  -> 任务监控快照
  -> SSE revision
  -> Pinia store
```

四类状态的职责是：Aria2 执行下载，Rust 内存提供运行态，SQLite 保存任务和维护记录，文件系统保存用户数据。任务操作表 `task_operations` 记录不能放进 SQLite 事务的外部副作用和阶段。

### 3.3 已成立的架构优势

- 管理面和 JSON-RPC 面分端口，JSON-RPC 默认只对本机开放。
- Web Session/CSRF 与 JSON-RPC Token 分离；密码使用 Argon2id，Token 不返回给普通前端设置接口。
- 任务、文件、Aria2、SQLite 和前端职责边界清楚，创建、控制、删除、恢复和重新下载共享任务级互斥。
- 进程 PID 身份、启动时间、退出清理、Aria2 session 保存和双 listener 绑定均有实现与测试。
- BT 任务使用任务专属目录和应用私有 metadata；`owned_task_dir` 让删除和恢复不依赖用户输入拼接路径。
- 1.8.1 的 FPK 身份、桌面入口和产物命名统一为 `motrix` / `motrix.Application` / `motrix_<version>_<arch>.fpk`。

## 4. 当前问题与证据

### 4.1 P0：用户文件安全（F-01，已修复）

| 结论 | 证据 | 影响 | 建议 |
| --- | --- | --- | --- |
| 重新下载不会在新任务可靠建立前删除旧文件 | `server/src/tasks/service/control.rs` 先创建暂停任务并持久化新 GID，再调用文件暂存；`server/src/tasks/files.rs` 将文件或 BT 专属目录移到同文件系统临时目录 | Aria2 拒绝、数据库失败、移动失败或恢复失败时，原任务快照和原文件可回滚 | 继续保留 `redownload_*` 测试；实机发布前验证权限撤销、只读目录和进程中断 |
| BT 重新下载按来源使用 `addTorrent` | `server/src/tasks/service/control.rs`、私有 metadata 和 `owned_task_dir` 路径 | 不再把种子当 URL 发送，恢复可保留 BT 元数据 | 源 metadata 缺失时维持“拒绝恢复/人工处理”，不要静默创建错误任务 |
| 同一任务不会同时执行冲突操作 | `TaskMemoryState` 任务锁覆盖重新下载及其他任务控制操作 | 重复提交得到冲突响应，不会并行移动同一份文件 | 保持服务端互斥，不能只依赖前端按钮禁用 |

### 4.2 P1：可靠性与一致性（代码整改完成）

| 结论 | 证据 | 影响 | 发布前建议 |
| --- | --- | --- | --- |
| 任务操作可记录、补偿和启动对账 | `server/src/database/task_operations.rs`、`server/src/runtime/task_operation_reconcile.rs`；记录阶段、GID、路径、外部副作用和错误 | 重启后能区分已持久化、未持久化和未知结果；无法安全判断时保留文件并转人工处理 | 用真实 Aria2 超时、服务崩溃和磁盘异常验证，不自动重复副作用 |
| 数据库迁移有版本，相关任务写入有事务边界 | `server/src/database/mod.rs` 的 `schema_migrations` 和有序迁移；任务、历史、错误与操作记录在同一业务事务中提交 | 不再依赖散落启动逻辑，失败时不留下半成品关联记录 | 在复制的历史库上做升级、重复启动和异常回滚演练 |
| Aria2 RPC 有统一客户端和未知结果分类 | `server/src/aria2/rpc.rs` 复用 `reqwest::Client`，统一连接/请求超时、响应校验和错误分类 | 断连或超时不会被误判为“肯定没执行”，也不会盲目重试创建 | 补“服务端已执行但客户端超时”和“请求未送达”两类设备/网络证据 |
| HTTP、上传和 WebSocket 有资源边界 | 管理/JSON-RPC body 1 MiB，种子上传总请求 12 MiB、文件 10 MiB，WebSocket 消息与写缓冲有上限；有限请求才使用超时 | 降低慢请求和大帧占满进程内存的风险，SSE/WebSocket 长连接不会被通用超时误杀 | 在慢客户端、畸形 multipart、超大帧和连接并发下观察回收 |
| 启动判断包含业务 readiness | `/api/app/ready` 检查退出状态、SQLite、管理 listener 和 RPC listener；`cmd/start`、`cmd/status` 同时核对 PID 身份和就绪状态 | 进程活着但端口或数据库未准备好时不会被误报为正常 | 在目标 fnOS 验证 curl/wget、超时、端口冲突和升级重启提示 |
| SSE 和前端 HTTP 结果有版本/代次保护 | 任务快照带单调递增 `revision`；SSE lag 发送完整快照；前端使用 `AbortController`、请求代次和版本过滤 | 迟到 HTTP 或旧 SSE 不会覆盖新状态，重连后会刷新 | 在浏览器断网、重连、慢响应和多标签页做端到端验证 |

P1 的边界很明确：数据库事务不包含 Aria2 和文件系统。代码采用“先记账、可补偿、启动对账、未知结果保守处理”，不是承诺跨系统原子提交。

### 4.3 P2：治理与平台验证

#### 已完成的治理项

| 领域 | 结论 | 代码/文档证据 | 影响 |
| --- | --- | --- | --- |
| 数据库参数和索引 | 已显式固定 SQLite WAL、busy timeout、同步级别和连接初始化；新增任务、历史、错误和未完成操作查询索引 | `server/src/database/mod.rs` 及新增迁移和测试 | 并发写入行为可重复，常用查询不依赖全表扫描 |
| 数据库维护 | 提供 `database-check`、`database-backup <output>` 和 `database-cleanup-history <timestamp> [--apply]` 内部命令 | `server/src/app/mod.rs`、`server/tests/database_cli.rs` | 健康检查和一致快照不暴露新的管理 HTTP API；清理默认 dry-run，事务失败会回滚 |
| 登录限速 | 失败桶按来源隔离，同时保留实例级总上限、来源数量上限和过期清理 | `server/src/auth/rate_limit.rs` 及测试 | 一个来源被锁定不会直接锁住所有来源；来源 Header 仍不能直接信任 |
| 可信代理与 Cookie | `MOTRIX_TRUSTED_PROXY_IPS` 命中真实对端地址后才读取来源 Header；`MOTRIX_WEB_COOKIE_SECURE` 显式控制 Cookie `Secure`，默认关闭 | `docs/api-contract.md`、`docs/fpk-packaging.md`、认证测试 | 代理部署规则可配置；HTTPS 终止场景必须显式打开 Secure，直连 HTTP 不会被错误判定 |
| 日志 | URL query/fragment、Token、密码、Session 和 CSRF 等敏感字段经过统一 redactor；响应带服务端 `X-Request-ID`；文件日志有大小上限和固定轮转数 | `server/src/debug_logs/`、`server/src/api/mod.rs`、`docs/fpk-packaging.md`、`docs/api-contract.md` | 调试和取证更容易关联，日志不会无限增长；集中收集和权限管理仍属运维责任 |
| CI 与供应链 | Node/pnpm/Rust/target 固定；第三方 Action 使用 commit SHA；Rust/前端生产依赖审计；Release 生成双架构 SBOM、SHA256SUMS 和 provenance | `.node-version`、`rust-toolchain.toml`、`.github/workflows/`、`scripts/dependency-audit.mjs` | 构建输入和发布产物可追溯，高危/严重依赖漏洞阻断发布 |
| FPK 预检 | x86_64/ARM64 产物名、manifest 身份、端口、回环 RPC 地址和运行时残留有自动检查 | `scripts/build-fpk.mjs`、`docs/fpk-packaging.md`、脚本测试 | 本地能提前发现包身份和架构错配，但不代替设备安装 |
| 依赖版本 | Rust 仅将确认有收益的 `spin` 从 `0.9.8` 更新到 `0.9.9`；前端候选含大版本和普通补丁，当前审计无漏洞且无明确兼容收益，保持不变 | `server/Cargo.lock`、`pnpm-lock.yaml`、审计结果 | 避免无必要的大范围升级；后续按安全公告或兼容需求逐项处理 |

#### 仍需处理的 P2 边界

| 风险 | 当前影响 | 建议完成标准 |
| --- | --- | --- |
| fnOS 实机证据 | 本地脚本不能证明安装、升级、重启、卸载、回滚和数据保留真实可用 | 在目标设备完成三个验收模板，保留版本、SHA-256、状态输出、日志和脱敏截图 |
| 代理部署 | Cookie `Secure` 默认关闭；未配置可信代理时伪造的 `X-Forwarded-For` 会被忽略，但部署者仍可能配置错误 | 反向代理只指向 `127.0.0.1:17081` 的 JSON-RPC，HTTPS 终止时显式设 Secure，并通过伪造 Header 测试 |
| 数据库运维 | 备份和清理命令已存在，但定期备份、保留期限、恢复演练和容量告警还没有统一运维安排 | 确定执行人、周期、备份位置、恢复校验和保留策略，不删除任务操作记录或用户文件 |
| 日志运维 | 脱敏和轮转已实现，集中收集、权限、归档和关联 ID 的长期保存策略仍需落地 | 在运行维护文档中确定目录、保留数量、收集方式和取证时的脱敏要求 |
| UI 设计门禁 | scoped CSS 外置、UnoCSS 试点和蓝色品牌迁移已完成；整体 UI 重设计尚未获得批准 | 用户批准 Figma frame 后再单独制定 UI 实施计划；此前不生成其他页面、不改交互、不安装运行时依赖 |

## 5. 整改路线

| 顺序 | 目标 | 完成标准 |
| --- | --- | --- |
| 1 | 保持用户文件安全 | 重新下载先建立暂停任务，文件暂存后再恢复；任何未知结果保留文件 |
| 2 | 保持本地状态可恢复 | 操作记录、版本化迁移、任务锁、补偿和启动对账持续覆盖所有任务动作 |
| 3 | 完成发布证据 | 本地完整验证通过，并取得 fnOS 安装/升级、生命周期、FN Connect/代理的脱敏证据 |
| 4 | 固化运维 | 运行数据库健康检查、备份、恢复、日志收集和依赖审计的周期与责任 |
| 5 | 推进产品治理 | 取得 UI 母版批准后再实施视觉重设计；依赖只按安全或兼容收益逐项升级 |

## 6. 发布门槛

### 本地门槛

```text
pnpm run version:check
pnpm run test:scripts
pnpm run typecheck
pnpm run test:unit
RUSTFLAGS='-D warnings' cargo test --manifest-path server/Cargo.toml
pnpm run verify:pre-commit
pnpm run verify
pnpm run build:fpk:prepare
git diff --check
```

本地通过只说明代码、脚本和预组装结果可重复，不说明 fnOS 入口、权限、代理或升级行为已经通过。

### 实机门槛

- 全新安装和从旧版同身份升级后，任务、设置、SQLite、Aria2 session 和首次登录/创建任务正常。
- `start`、`status`、`stop`、设备重启、卸载和回滚不留下错误 PID、端口或 sidecar，也不误删用户文件。
- FN Connect 短域名进入管理端口；Lucky/其他反向代理只进入回环 JSON-RPC 端口；SSE、WebSocket、Cookie 和请求关联 ID 符合配置。
- 证据不记录密码、Session、CSRF、JSON-RPC Token 或完整私密 URL。

## 7. 最终结论

以 `35b3a8e` 为基线的 1.8.1 代码已经解决重新下载文件丢失、任务操作不可追踪、数据库迁移分散、RPC 超时无分类、启动只看进程存活、SSE/HTTP 乱序覆盖以及主要治理缺口。当前架构清晰，继续在现有 Rust/Axum、SQLite、Aria2 和 Vue/Pinia 组合上演进即可。

尚未闭环的事项集中在“证据和运维”而不是代码大改：目标 fnOS 设备的安装生命周期、真实代理链路、外部副作用故障演练、数据库恢复制度、日志集中治理和 UI 母版批准。没有这些证据时，报告只把本地自动化标为通过，不把平台发布标为完成。
