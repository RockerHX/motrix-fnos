# 管理面鉴权与 JSON-RPC 隔离迁移记录

> 本文是 2026-07-14 安全改造计划的归档记录，不是当前开发计划。
> 当前事实以 [`architecture.md`](architecture.md)、[`api-contract.md`](api-contract.md) 和 [`fpk-packaging.md`](fpk-packaging.md) 为准；未完成事项统一见 [`future-development-plan.md`](future-development-plan.md)。

## 迁移前问题

旧版本只监听 `0.0.0.0:17080`，Web UI、管理 API 和 `/jsonrpc` 共用一个入口。Lucky 反代到该端口时，管理面也会暴露；JSON-RPC Token 不能保护任务、设置和日志 API。

## 当前已落地边界

```text
管理面       0.0.0.0:17080  Web UI、HTTP API、SSE
回环 RPC     127.0.0.1:17081  仅 /jsonrpc，供本机反向代理
局域网 RPC   0.0.0.0:17082  仅 /jsonrpc，RFC1918 IPv4 + 独立 Token
```

- `17080` 不注册 `/jsonrpc`；管理 API 使用 Web Session 和 CSRF。
- `17081` 只绑定回环地址，未知路径返回 `404`，不得进入 FPK 端口映射。
- `17082` 始终监听，但入口关闭或来源不符合 RFC1918 IPv4 时拒绝请求；它不能复用回环 Token。
- Lucky 只应反向代理到 `http://127.0.0.1:17081`。
- Web 管理密码、Web Session、CSRF Token、回环 RPC Token 和局域网 RPC Token 相互独立。
- 管理 API 的未登录请求返回 `401`；`/api/app/ready` 是用于生命周期探测的匿名就绪接口。

## 已完成的验证范围

- Rust 路由测试覆盖三个监听器的路径隔离、回环地址约束、局域网真实 TCP 对端和 Token 隔离。
- 前端和 API 测试覆盖登录、Session 失效、CSRF 以及敏感信息不回显。
- FPK 预检确认 `17080` 和 `17082` 进入端口声明，`17081` 不进入 manifest、桌面入口或端口映射。
- 本地 `pnpm run verify` 已通过；fnOS 正式身份和 FPK 实机矩阵仍属于后续验收，不在本地测试中虚构为已完成。

## 不兼容变化

- 旧的 `17080/jsonrpc` 入口已移除。
- Lucky、解析站或其他公网调用方必须改用 `127.0.0.1:17081/jsonrpc` 的反向代理地址。
- 管理面首次使用需要设置独立 Web 管理密码，不能把 JSON-RPC Token 当作 Web 密码。

历史计划中的实现步骤、旧端口示例和验收表不再作为操作指南；保留本文件仅用于解释端口迁移和安全边界的来源。
