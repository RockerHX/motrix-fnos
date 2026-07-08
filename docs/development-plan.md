# 飞牛版 Motrix 开发计划

> 更新时间：2026-07-08
> 本文档记录当前阶段状态、已完成里程碑、优先级和验收口径。长期架构边界见 `docs/architecture.md`；HTTP / SSE 与 JSON-RPC 接口见 `docs/api-contract.md`；FPK 构建与产物见 `docs/fpk-packaging.md`。

## 1. 当前状态

版本来源：以 `package.json`、`server/Cargo.toml` 与 `packaging/fnos/manifest.template` 为准，Release tag 使用 `v<version>`。

当前阶段：**阶段 12 已完成，发布前验收与缺陷修复中；下一阶段待定（✅ 2026-07-07）**

阶段摘要：

- 当前发布主线已覆盖 Rust server、Vue Web UI、Aria2 Next sidecar、SQLite 与 FPK 打包链路。
- 当前主线已完成任务管理、设置、诊断日志、应用内国际化、手机端 UI 适配和关于页能力。
- 新建任务链路已补齐单 URL、批量 URL、种子文件、磁力链接、开始方式和高级下载选项；发布前仍需按手工验收清单在目标 fnOS 环境复核。
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
| 阶段 12：新建下载任务能力完善 | ✅ 2026-07-07 | 已补齐 URL / 批量 URL / Multipart 种子文件 / 磁力链接、立即开始 / 添加后暂停、分类、连接数、下载限速和代理的前后端能力。 |

## 3. 当前优先级

阶段 12 已完成；当前进入发布前验收、缺陷修复和发版准备。下一阶段尚未启动，除紧急修复外，新功能应先更新本计划再实施。

阶段 12 完成摘要：

- 新建任务入口已覆盖单 URL、批量 URL、Multipart 种子文件和磁力链接。
- 开始方式、分类、连接数、下载限速和代理高级选项已接入前后端。
- 磁链流程已支持 metadata 解析、真实文件确认、部分文件选择、解析种子元数据保存和任务专属目录清理。

后续维护阶段 12 能力时仍应确认是否影响以下边界：

- 是否新增或改变 HTTP / SSE / JSON-RPC 契约。
- 是否新增长期状态、数据库字段或迁移。
- 是否涉及 fnOS / FPK 生命周期、权限、端口入口或文件夹授权行为。
- 是否需要更新 `docs/architecture.md` 的长期架构约束。

发布前手工验收清单：

- HTTP/HTTPS、批量 URL、Multipart 种子文件和磁力链接均可创建任务。
- 磁链可完成 metadata 解析、文件确认、部分文件选择、确认后开始下载，并在任务专属目录生成 Aria2 hash 命名 `.torrent`。
- URL、磁力链接和种子文件入口的立即开始 / 添加后暂停均生效。
- 分类、连接数、下载限速、代理在新建任务时生效，任务记录可见分类。
- 删除任务并勾选删除文件时，普通文件、种子任务目录和磁链任务目录均按预期清理。
- fnOS 授权目录校验仍拦截未授权保存路径。

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
- `docs/development/archive/`：阶段实施细节的历史归档，不作为当前优先级来源。
- `docs/api-contract.md`：HTTP / SSE / JSON-RPC 接口契约的唯一来源。
- `docs/fpk-packaging.md`：FPK 构建命令、产物位置和打包排障入口的唯一来源。
- `docs/design/archive/ui-stitch-prompts.md`：历史设计归档参考，不作为当前实现契约。
- `CHANGELOG.md`：发布历史。
