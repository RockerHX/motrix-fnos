# FN Connect 与反向代理验收记录

状态：待目标 fnOS 实机和真实代理环境验证。本文档是证据模板，不代表当前版本已经通过公网链路验收。

## 基本信息

| 项目 | 记录 |
| --- | --- |
| FPK 版本 | `1.8.1` |
| 设备型号 / CPU | 待填写 |
| fnOS 版本 | 待填写 |
| FN Connect 短域名 | 待填写（不要记录登录凭据） |
| Lucky / 反向代理版本 | 待填写 |
| 验证时间 | 待填写 |
| 验证人 | 待填写 |

## FN Connect 短域名

1. 在设备本地确认 FPK 身份为 `motrix`，桌面入口为 `motrix.Application`，`desktop_applaunchname` 为空。
2. 通过 `motrix.<account>.fnos.net` 打开管理页面，确认请求进入管理端口（默认 `17080`）。
3. 完成首次登录、任务列表加载、任务创建和 SSE 进度更新。
4. 确认管理页面不要求把 JSON-RPC Token 放入 URL、Cookie 或普通响应。

通过标准：短域名可以打开管理 UI，登录和 SSE 正常；公网请求不能直接访问 JSON-RPC 专用端口或管理 API 的未授权路径。

## Lucky 或其他反向代理

1. 在设备本机确认 `127.0.0.1:17081/jsonrpc` 可用，且 `17080` 仍是管理 listener。
2. 将代理后端只指向回环 JSON-RPC listener，不把公网流量转发到管理端口。
3. 使用合规 JSON-RPC Token 测试 `getVersion` 和一个只读任务查询；记录 HTTP、WebSocket 和 CORS 预检结果。
4. 通过代理访问管理 UI，确认 `X-Request-ID`、Session Cookie、`Secure` 配置、SSE 和 WebSocket 行为符合部署设置。
5. 使用伪造的 `X-Forwarded-For`、`X-Forwarded-Proto` 和同名 `X-Request-ID`，确认服务不盲目信任这些 Header。

通过标准：管理端口与 JSON-RPC 端口没有混淆；代理下登录、Cookie、SSE、WebSocket 和 JSON-RPC 均按配置工作；不可信 Header 不能绕过来源限速、Cookie 策略或请求关联 ID。

## 证据清单

- [ ] FN Connect 短域名访问截图和设备端请求日志
- [ ] 两个 listener 的本机监听证明（隐藏公网地址和凭据）
- [ ] Lucky/代理后端配置截图或脱敏导出
- [ ] HTTP、WebSocket、CORS、SSE 和登录 Cookie 测试结果
- [ ] 伪造 Header 测试结果
- [ ] 失败请求的 `X-Request-ID` 与 `server.log` 对应记录
