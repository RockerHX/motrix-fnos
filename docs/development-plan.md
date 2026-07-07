# 飞牛版 Motrix 开发计划

> 更新时间：2026-07-07  
> 本文档记录当前阶段状态、已完成里程碑、优先级和验收口径。长期架构边界见 `docs/architecture.md`；HTTP / SSE 与 JSON-RPC 接口见 `docs/api-contract.md`；FPK 构建与产物见 `docs/fpk-packaging.md`。

## 1. 当前状态

版本来源：以 `package.json`、`server/Cargo.toml` 与 `packaging/fnos/manifest.template` 为准，Release tag 使用 `v<version>`。

当前阶段：**阶段 12：新建下载任务能力完善（🟡 规划中）**

阶段摘要：

- 当前发布主线已覆盖 Rust server、Vue Web UI、Aria2 Next sidecar、SQLite 与 FPK 打包链路。
- 当前主线已完成任务管理、设置、诊断日志、应用内国际化、手机端 UI 适配和关于页能力。
- 当前新建任务链路仅覆盖单 HTTP/HTTPS URL 的基础创建；批量 URL、种子文件、磁力链接、开始方式和高级下载选项需要进入下一阶段补齐。
- FPK 仍按设备 CPU 架构区分 x86 与 ARM 两个产物；具体构建命令与产物路径见 `docs/fpk-packaging.md`。
- 桌面 Web、手机浏览器和飞牛 App WebView 继续共用同一套 Vue 源码、Pinia store、service、HTTP API 和 SSE 数据流。

## 2. 阶段里程碑

| 阶段 | 状态 | 完成结论 |
| --- | --- | --- |
| 阶段 0：架构纠偏 | ✅ 2026-07-01 | 收口为 FPK / Rust server / Vue Web UI / Aria2 sidecar 主线，并建立架构、计划、API 和打包文档边界。 |
| 阶段 1：抽出 Rust 业务核心 | ✅ 2026-07-02 | `server/` 成为业务核心承载地，配置、日志、数据库、任务、Aria2 和设置能力进入 server 主线。 |
| 阶段 2：HTTP API 和事件流 | ✅ 2026-07-02 | `/api/*`、统一错误响应、任务/设置/日志/Aria2 接口和 `/api/events` SSE 已落地。 |
| 阶段 3：前端迁移到 HTTP API | ✅ 2026-07-02 | Vue UI 作为普通 Web UI 运行，通过 `fetch` 与 `EventSource` 消费后端。 |
| 阶段 4：FPK 打包链路 | ✅ 2026-07-02 | `packaging/fnos/`、服务脚本、静态资源、server 和 Aria2 sidecar 已纳入组装流程。 |
| 阶段 5：飞牛实机验证 | ✅ 2026-07-03 | 匹配架构的 FPK 可安装运行，核心下载、设置、日志、停止和 session 恢复可用。 |
| 阶段 6：侧栏分类菜单功能化 | ✅ 2026-07-03 | Downloading / Completed / Stopped / Trash / Extensions 可切换，Trash 查询和永久删除记录已完成。 |
| 阶段 7：设置项与帮助入口 | ✅ 2026-07-05 | 设置页只保留已生效能力，帮助弹窗和诊断状态同步已补齐，前端缓存策略通过入口版本参数与 no-cache 响应头收口。 |
| 阶段 8：数据目录生命周期 | ✅ 2026-07-05 | 升级和卸载默认保留 `TRIM_PKGVAR` 应用数据；卸载向导可选删除 Motrix 私有运行数据，不删除用户下载文件。 |
| 阶段 9：应用内国际化 | ✅ 2026-07-05 | Web UI 支持简体中文 / English 手动切换并通过设置持久化；平台外壳保持中文。 |
| 阶段 10：手机端 UI 适配 | ✅ 2026-07-05 | 移动端切换为单列外壳、底部导航、任务卡片和移动端弹窗；桌面布局保持不回退。 |
| 阶段 11：关于页、版本检测与更新历史 | ✅ 2026-07-05 | 新增关于页入口，展示应用信息、版本检测结果、手动更新说明和 `CHANGELOG.md` 更新历史。 |
| 阶段 12：新建下载任务能力完善 | 🟡 规划中 | 补齐新建任务弹窗中 URL / 批量 URL / 种子文件 / 磁力链接、开始方式和高级选项的前后端能力。 |

## 3. 当前优先级

当前开发重点切换为阶段 12：完善“新建下载任务”能力。该阶段应以现有 HTTP API / SSE / Aria2 JSON-RPC 主线为基础，复用 `features/tasks` 的 store/service/composable，不引入独立前端状态或绕过后端直接访问 Aria2。

阶段 12 小任务清单：

- [x] **小任务 0：落地阶段 12 可勾选清单**
  - 在本文档记录阶段 12 的可勾选实施清单、验收命令和提交规范。
  - 验收命令：`rtk pnpm run verify:pre-commit`。
  - 提交信息：`docs: 细化阶段 12 新建任务实施清单`。
- [x] **小任务 1：扩展任务模型、分类字段和接口契约**
  - 后端 `DownloadTask` / 前端 `DownloadTask` 增加 `category`，SQLite 新库和老库迁移均提供默认分类 `默认`。
  - 新建任务请求类型加入 `sourceType`、`startMode`、`category`、`advancedOptions`，并保持旧请求兼容。
  - 同步更新 `docs/api-contract.md`。
  - 验收命令：`rtk cargo test --manifest-path server/Cargo.toml database`、`rtk pnpm test:unit -- src/features/tasks/stores/taskStore.spec.ts`。
  - 提交信息：`feat: 扩展新建任务契约和任务分类字段`。
- [x] **小任务 2：支持单任务磁力链接和添加后暂停**
  - `sourceType=url` 仅接受 HTTP/HTTPS，`sourceType=magnet` 仅接受 `magnet:?`。
  - `POST /api/tasks` 支持磁力链接；`startMode=paused` 映射 Aria2 暂停选项并持久化为暂停态。
  - JSON-RPC `aria2.addUri` 复用同一套选项过滤并接受 HTTP/HTTPS 与磁力链接。
  - 验收命令：`rtk cargo test --manifest-path server/Cargo.toml tasks::tests`、`rtk cargo test --manifest-path server/Cargo.toml api::tasks::tests`、`rtk cargo test --manifest-path server/Cargo.toml api::jsonrpc::tests`。
  - 提交信息：`feat: 支持磁力链接和添加后暂停`。
- [x] **小任务 3：支持批量 URL 创建**
  - 新增 `/api/tasks/batch`，逐条校验和创建，部分失败不回滚已创建任务。
  - 前端 service/store 增加批量创建能力，成功任务写入任务列表。
  - 验收命令：`rtk cargo test --manifest-path server/Cargo.toml api::tasks::tests`、`rtk pnpm test:unit -- src/features/tasks/stores/taskStore.spec.ts`。
  - 提交信息：`feat: 支持批量 URL 创建任务`。
- [ ] **小任务 4：支持 Multipart 种子文件创建**
  - 新增 `/api/tasks/torrent`，接收 `torrent` 文件和 `request` JSON 字段，限制 torrent 文件不超过 10 MiB。
  - 后端调用 Aria2 `addTorrent`，不持久化种子原文件。
  - 验收命令：`rtk cargo test --manifest-path server/Cargo.toml tasks::tests`、`rtk cargo test --manifest-path server/Cargo.toml api::tasks::tests`。
  - 提交信息：`feat: 支持种子文件上传创建任务`。
- [ ] **小任务 5：接入高级设置四项**
  - 分类作为任务标签持久化，不改变保存路径和侧栏状态分类。
  - 连接数、下载限速和代理映射为受控 Aria2 options，并集中校验过滤。
  - 验收命令：`rtk cargo test --manifest-path server/Cargo.toml tasks::tests`、`rtk cargo test --manifest-path server/Cargo.toml api::tasks::tests`、`rtk cargo test --manifest-path server/Cargo.toml api::jsonrpc::tests`。
  - 提交信息：`feat: 接入新建任务高级设置`。
- [ ] **小任务 6：完善新建任务弹窗交互**
  - 启用 URL、批量 URL、种子文件、磁力链接四个 Tab。
  - 公共区域接入保存路径、开始方式、分类、连接数、限速、代理；移除未持久化备注。
  - 批量部分失败时保留弹窗并展示失败列表，全部成功时重置并关闭。
  - 验收命令：`rtk pnpm test:unit -- src/features/tasks/composables/useTaskCreateForm.spec.ts`、`rtk pnpm test:unit -- src/features/tasks/components/TaskCreateDialog.spec.ts`、`rtk pnpm run typecheck`。
  - 提交信息：`feat: 完善新建下载任务弹窗`。
- [ ] **小任务 7：阶段 12 收口验证**
  - 运行快速总验证，必要时运行完整验证。
  - 将阶段 12 状态改为已完成，并补充发布前手工验收清单。
  - 验收命令：`rtk pnpm run verify:pre-commit`、必要时 `rtk pnpm run verify`。
  - 提交信息：`docs: 标记阶段 12 新建任务能力完成`。

阶段 12 实现前仍应确认是否影响以下边界：

- 是否新增或改变 HTTP / SSE / JSON-RPC 契约。
- 是否新增长期状态、数据库字段或迁移。
- 是否涉及 fnOS / FPK 生命周期、权限、端口入口或文件夹授权行为。
- 是否需要更新 `docs/architecture.md` 的长期架构约束。

## 4. 验证口径

常规自动验证：

```bash
rtk pnpm run verify:pre-commit
```

发布前补充验证：

- `rtk pnpm run verify`
- 按设备架构安装对应 FPK。
- 启动服务并打开 Web UI。
- 验证 `/api/app/ping`、任务列表、SSE 刷新和退出态提示。
- 验证 HTTP/HTTPS 下载、暂停、继续、删除、回收站和永久删除记录。
- 验证新建任务弹窗的单 URL、批量 URL、磁力链接、种子文件、立即开始、添加后暂停和已接入高级选项。
- 验证设置保存、JSON-RPC token、诊断日志查看与清空。
- 验证停止服务后的 session 保存和重启恢复。
- 验证手机端首屏、分类导航、创建任务、任务卡片、设置、帮助、关于、诊断和日志弹窗。
- 验证关于页应用信息、版本检测失败回退、Release 链接和更新历史展示。
- 验证旧版本升级到新版本后，任务、设置、JSON-RPC 密钥和 Aria2 session 保留。
- 验证卸载时不勾选删除数据会保留 `TRIM_PKGVAR`；勾选删除数据仅清理 Motrix 应用数据，不删除用户下载文件。

## 5. 文档关系

- `docs/architecture.md`：长期架构边界和分层约束的唯一来源。
- `docs/development-plan.md`：当前阶段状态、里程碑、优先级和验收口径的唯一来源。
- `docs/api-contract.md`：HTTP / SSE / JSON-RPC 接口契约的唯一来源。
- `docs/fpk-packaging.md`：FPK 构建命令、产物位置和打包排障入口的唯一来源。
- `docs/design/archive/ui-stitch-prompts.md`：历史设计归档参考，不作为当前实现契约。
- `CHANGELOG.md`：发布历史。
