# Motrix fnOS 项目分析报告

> 分析日期：2026-07-12  
> 范围：Vue 前端、Rust 服务、Aria2、FPK 脚本、测试与 CI

## 总体结论

项目主架构清晰，前后端分层和自动测试基础较完整，目前可以正常构建和测试。主要风险集中在文件删除安全、服务访问控制和 FPK 进程管理；另外存在少量维护性与 CI 覆盖问题。

## 发现的问题与解决方案

### 1. BT 任务目录存在符号链接误删风险（高）

**状态**：已修复（2026-07-13）。删除逻辑会在递归删除前拒绝符号链接形式的 BT 任务目录，并已增加链接目标保留测试。

**问题**：`server/src/tasks/files.rs` 删除 BT 任务目录时，会先解析真实路径，再按目录名判断是否为任务目录。如果原任务目录被替换成同名符号链接，理论上可能递归删除链接指向的其他目录。

**影响**：勾选“删除文件”时可能误删非任务数据。

**解决方案**：

- 删除前使用 `symlink_metadata` 明确拒绝符号链接。
- 保存并校验任务创建时的授权父目录和真实任务目录。
- 删除时同时验证“目录位于授权父目录下”和“目录就是该任务创建的目录”，不要只比较目录名。
- 增加符号链接、路径替换、父目录越界测试。

### 2. `/api/*` 缺少统一访问控制（高）

**状态**：已修复（2026-07-13）。FPK 的 Web UI、`/api/*` 与 SSE 已迁移到 fnOS 统一网关，并要求已登录管理员身份；独立 TCP 端口只保留带 token 的 `/jsonrpc`。

**查证依据**：飞牛官方《[应用入口](https://developer.fnnas.com/docs/core-concepts/app-entry/)》说明端口服务与 NAS 登录态无关；《[统一网关](https://developer.fnnas.com/docs/core-concepts/gateway-registration/)》说明网关会校验用户会话并转发用户身份 Header。

**问题**：FPK 默认监听 `0.0.0.0`，普通 `/api/*` 接口没有统一鉴权。JSON-RPC 的新增任务接口有 token 校验，但任务删除、设置修改、日志读取、Aria2 启停等管理接口主要依赖网络环境隔离。

**影响**：如果服务端口可被局域网或非预期页面访问，其他客户端可能操作下载任务或读取运行信息。

**解决方案**：

- 优先确认 fnOS 入口代理是否能提供可靠的登录态校验。
- 若不能，给 `/api/*` 和 SSE 增加统一会话/token 鉴权，并限制 WebSocket/SSE 来源。
- 对修改类请求增加 `Origin`/CSRF 防护；不需要跨域的接口不要开放宽泛 CORS。
- 明确记录“端口是否允许局域网直连”的产品策略和实机验证结果。

### 3. PID 文件可能命中已复用的其他进程（中）

**状态**：已修复（2026-07-13）。FPK 脚本现会同时校验 PID、`/proc/<pid>/exe` 和进程启动时间；启动成功还要求统一网关 Socket 已就绪。旧版单 PID 记录会在可执行文件身份一致时兼容读取。

**验证方式**：新增 `scripts/test-fnos-process-identity.sh`，模拟 PID 相同但启动时间或可执行文件已变化的场景，确认不会误判或向无关进程发送信号。

**问题**：`packaging/fnos/cmd/common.sh` 的进程判断只有 `kill -0 <pid>`。服务异常退出后，如果 PID 被系统复用，`status` 会误报，`stop` 可能向无关进程发送 `SIGINT`。

**影响**：状态判断不准确，极端情况下会影响其他进程。

**解决方案**：

- 校验 `/proc/<pid>/exe` 或 `/proc/<pid>/cmdline` 是否对应 `motrix-fnos-server`。
- PID 文件中同时保存启动时间或进程身份信息，并在 stop/status 时复核。
- 启动成功判断改为请求 `/api/app/ping`，不要只判断进程仍存在。

### 4. 核心任务模块体积偏大（中）

**问题**：`server/src/tasks/service.rs` 超过 1000 行，集中处理创建、磁链解析确认、暂停恢复、删除、重下和持久化同步；`src/views/MainWindow.vue` 也承担了较多批量操作和页面状态编排。

**影响**：修改任务流程时影响面较大，代码审查和回归定位成本会上升。

**解决方案**：

- 后端按 `create / control / delete / magnet-confirm` 拆分服务，但保留统一任务创建底层链路。
- 前端将批量任务操作继续下沉到 `features/tasks` 的 composable，`MainWindow.vue` 只负责页面组合。
- 拆分时保持现有接口不变，并以当前测试作为回归基线。

#### 拆分实施任务

按以下顺序逐项实施，每项应独立提交，避免一次重构范围过大。

1. **整理后端测试文件**
   - 将 `server/src/tasks/service.rs` 内的测试、Fake Repository 和 Mock Aria2 移到 `server/src/tasks/service/tests.rs`。
   - 只移动代码，不改变生产逻辑。
   - 验收：Rust 测试数量和结果保持不变。

2. **抽离任务查询与持久化同步**
   - 新建 `server/src/tasks/service/query.rs`，承载任务列表、回收站列表和 Aria2 刷新后的 SQLite 同步。
   - `TaskService` 保留原公开方法，内部委托给新模块。
   - 验收：`GET /api/tasks`、回收站查询、SSE 任务快照行为不变。

3. **抽离任务删除与回收站流程**
   - 新建 `server/src/tasks/service/delete.rs`，承载普通删除、删除本地文件、磁链临时目录清理和永久删除记录。
   - 继续复用现有路径安全与符号链接保护逻辑。
   - 验收：删除、勾选删除文件、回收站永久删除相关测试全部通过。

4. **抽离暂停、恢复与重新下载流程**
   - 新建 `server/src/tasks/service/control.rs`，承载暂停、恢复、失效 GID 重建和重新下载。
   - 保持现有 Aria2 调用顺序、错误信息和持久化时机不变。
   - 验收：暂停、恢复、失效 GID、重新下载测试全部通过。

5. **抽离普通 URL 与种子创建流程**
   - 新建 `server/src/tasks/service/create.rs`，承载 URL、批量 URL 和种子任务创建编排。
   - 保留统一的参数校验、Aria2 option 映射、内存写入和 SQLite 持久化链路。
   - 验收：单 URL、批量 URL、种子文件、立即开始和添加后暂停行为不变。

6. **抽离磁链解析与文件确认流程**
   - 新建 `server/src/tasks/service/magnet.rs`，承载 metadata 临时目录、解析状态和文件确认后的真实 BT 创建。
   - 不改变磁链临时目录、`.torrent` 保存和清理安全边界。
   - 验收：磁链解析、部分文件选择、确认下载、删除解析中任务和重启恢复测试全部通过。

7. **收口后端服务入口**
   - `server/src/tasks/service.rs` 只保留 `TaskService`、依赖注入、运行态守卫和对各子模块的公开委托。
   - 不修改 HTTP handler、API 路径、响应结构或错误码。
   - 验收：`service.rs` 生产代码控制在约 250 行内，完整验证通过。

8. **抽离前端批量任务操作**
   - 新建 `src/features/tasks/composables/useTaskBulkActions.ts`，迁移批量暂停、恢复、删除、清空回收站、确认弹窗和结果 Toast。
   - `MainWindow.vue` 只绑定 composable 返回的状态和事件。
   - 验收：补充 composable 单元测试，现有顶部工具栏与批量确认测试通过。

9. **抽离前端顶部操作状态**
   - 新建 `src/features/tasks/composables/useTaskTopbarActions.ts`，迁移按钮可用状态、禁用原因和刷新分流逻辑。
   - 保持 Extensions、Trash、运行时退出等特殊状态表现不变。
   - 验收：各分类下按钮状态、提示文本和刷新目标测试通过。

10. **抽离页面弹窗与启动刷新编排**
    - 新建 `src/views/composables/useMainWindowDialogs.ts` 管理创建、设置、帮助、关于和诊断弹窗。
    - 将首次任务刷新、Aria2 状态刷新和退出时关闭创建弹窗整理为独立页面 composable。
    - 验收：`MainWindow.vue` 只保留布局组合、分类切换和组件事件连接，控制在约 250 行内。

11. **最终回归与文档收口**
    - 运行 `pnpm run verify`，并按开发计划复核 URL、种子、磁链、任务控制、删除和移动端关键路径。
    - 若目录或职责边界发生变化，同步更新 `docs/architecture.md`；接口未变化时无需修改 API 契约。

### 5. CI 对部分版本文件变更会跳过验证（低）

**问题**：`.github/workflows/verify.yml` 对 `package.json`、`Cargo.toml`、锁文件和 manifest 等路径设置了 `paths-ignore`。当提交只修改这些文件时，普通 Verify 工作流不会运行。

**影响**：依赖、锁文件或版本配置的独立变更可能缺少自动验证。

**解决方案**：

- 只忽略纯文档或明确无运行影响的文件。
- 版本文件至少运行 `version:check`、依赖安装、Rust 编译和前端构建。
- 发布工作流保留完整构建与 FPK 产物校验。

## 验证结果

- `pnpm run verify:pre-commit`：通过。
- Rust：153 项测试通过，warnings 视为错误。
- 前端：类型检查通过，145 项测试通过。
- `pnpm audit --prod`：未发现已知生产依赖漏洞。

## 建议处理顺序

1. 修复 BT 目录删除的符号链接风险。
2. 明确并补齐 `/api/*` 的访问控制边界。
3. 加固 FPK PID 与健康检查逻辑。
4. 再处理模块拆分和 CI 路径规则。
