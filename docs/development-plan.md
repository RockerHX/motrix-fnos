# 当前开发计划

> 更新时间：2026-08-06
> 本文档记录当前独立功能修复的状态、实施门禁和验收口径。已完成内容见 `CHANGELOG.md`。
> 尚未进入当前开发计划的候选事项、长期待办和各自启动门禁见 `docs/future-development-plan.md`。

## 1. fnOS 生命周期与任务清理安全修复

状态：实施中，优先级高于后续功能开发。

实机日志已确认 1.8.5 在删除任务异常后出现停止失败、连续启动失败、应用中心 `TRPC read timeout`，并在卸载 stop 脚本失败后仍被平台记录为卸载成功。当前修复按以下顺序实施：

- 为 `cmd/stop` 增加基于 PID、启动时间和可执行文件归属的 `INT -> TERM -> KILL` 有界兜底，卸载停止失败时失败关闭。
- 为 Rust server 的 HTTP 排空、任务持久化和 Aria2 退出增加共享总预算，避免在途请求或 RPC 无限拖住生命周期。
- 把勾选删除文件后的递归清理改为持久化后台操作，HTTP 请求只负责原子暂存和回收站状态提交。
- 启动时定向清理能够证明归属的 server/Aria2 孤儿进程，无法证明归属的端口冲突只报告不误杀。

发布门禁包括 shell 身份校验与信号升级测试、Rust 慢清理/慢 RPC/退出超时测试、完整源码验证，以及 fnOS 上保留数据与删除数据两种卸载路径实测。实机验收前不得主动复现“删除大量文件后立即停用或卸载”。

## 2. 文档关系

- `docs/architecture.md`：长期架构与职责边界。
- `docs/api-contract.md`：HTTP、SSE 与 JSON-RPC 接口契约。
- `docs/fpk-packaging.md`：FPK 构建、发布和实机验证。
- `docs/future-development-plan.md`：尚未进入当前阶段的候选事项与启动门禁。
- `CHANGELOG.md`：已完成功能与发布历史。

## 3. 后期平台实验：统一网关可信性验证

状态：延期、非阻塞，不属于当前开发计划。当前 Motrix 继续使用已经实机验证的端口入口；本实验不得阻塞功能开发、常规修复或正式发布。

实验必须使用独立 appname 的最小 FPK，不得直接修改 Motrix 主包。最小包只包含：

- 一个 `gatewayPrefix` 与一个位于 `TRIM_APPDEST` 的 Unix Socket。
- 一个返回固定 HTML 的根路由和一个返回固定 JSON 的健康检查路由。
- 一个 SSE 或 WebSocket 回显路由，用于验证长连接转发。
- 记录请求 path、`X-Trim-Userid`、`X-Trim-Isadmin` 和 `X-Trim-Username` 是否存在的脱敏日志。

实机验证矩阵：

1. 全新安装后，确认 `trim_sac/open_gateway` 生成入口和转发表，登录用户访问公开路径返回 `200`。
2. 直接请求 Unix Socket 与通过 fnOS nginx 请求得到一致响应，并记录网关转发时 path 是保留还是剥离前缀。
3. 未登录请求被 fnOS 拒绝；已登录请求携带可信 `X-Trim-*` Header。
4. 根 HTML、子路径、静态资源、健康检查及 SSE/WebSocket 均能通过同一网关前缀访问。
5. 分别验证停止后启动、覆盖升级、卸载重装和设备重启，确认 Socket 清理与路由注册不会残留或丢失。
6. 收集 fnOS 版本、入口数据库记录、nginx access/error log、注册服务日志和最小 FPK 校验和，形成可复现报告。

迁移门禁：

- 最小 FPK 在目标 fnOS 实机上完整通过上述矩阵后，才能提出 Motrix 统一网关迁移方案。
- 迁移前必须先更新 `docs/architecture.md`、`docs/api-contract.md` 和 `docs/fpk-packaging.md`，并明确端口模式回退方案。
- Motrix 的候选实现必须单独验证 FPK 最终产物和真实 fnOS 转发链路；仅有 Axum Router 测试或 Unix Socket 直连测试不算通过。
- 任一注册、鉴权、路径或长连接场景失败时，Motrix 继续保持当前端口入口，不合并实验代码。
