# JSON-RPC 远程访问说明

本文档说明如何通过 `/jsonrpc` 兼容入口从外部工具添加下载任务。接口字段和错误码以 [`docs/api-contract.md`](api-contract.md#6-json-rpc-兼容入口) 为准；Web UI 主通信仍使用 `/api/*` 与 `/api/events`。

## 适用场景

- 解析站、浏览器扩展或自动化脚本希望按 Aria2 JSON-RPC 习惯远程添加 HTTP / HTTPS 或磁力链接下载任务。
- 外部工具只需要提交任务，不直接访问 Motrix 内部 Aria2 RPC secret。
- 需要通过 `aria2.getVersion` 做连通性测试。

## 鉴权 token

`aria2.addUri` 必须携带 `jsonRpcToken`：

1. 在 Web UI 设置页保存 JSON-RPC 密钥。
2. 调用 `/jsonrpc` 时，将第一个参数设为 `"token:<jsonRpcToken>"`。
3. `jsonRpcToken` 为空时，添加任务会被拒绝；`aria2.getVersion` 仍可匿名调用。

该 token 不是 Aria2 RPC Secret，后端不会把内部 Aria2 secret 暴露给前端或外部调用方。

## 最小示例

```bash
curl -X POST 'http://<host>:<port>/jsonrpc' \
  -H 'Content-Type: application/json' \
  -d '{
    "jsonrpc": "2.0",
    "id": "add-1",
    "method": "aria2.addUri",
    "params": [
      "token:your-json-rpc-token",
      ["https://example.com/file.zip"],
      {
        "dir": "/vol1/downloads",
        "out": "file.zip"
      }
    ]
  }'
```

成功时返回 Aria2 GID；任务状态仍由 Motrix 后端和 Web UI 管理。

## 保存目录约束

- `dir` 必须来自 `/api/storage/accessible-paths` 返回的 fnOS 已授权目录。
- 未传 `dir` 时使用后端默认下载目录，默认目录同样必须已授权。
- `out` 会映射为 Motrix 任务文件名；磁力链接不会设置 `out`。
- 磁力链接的 `dir` 表示授权父目录；后端会创建任务专属子目录，启用 metadata 暂停和 `bt-save-metadata`，解析出的 hash 命名 `.torrent` 仅用于可见性 / 排障，不替代 Aria2 session。
- 磁力链接解析完成后，真实 BT 下载仍需要在 Web UI 中确认文件；远程入口只负责添加任务。
- 支持 HTTP / HTTPS URL 与 `magnet:?` 磁力链接；不支持通过 JSON-RPC 上传种子文件，种子文件请使用 Web UI 或 `/api/tasks/torrent`。
- 常用 Aria2 选项会经过后端白名单过滤；`dir` / `out` 仍由 Motrix 统一校验和覆盖。

## 安全注意事项

- 不要把 `jsonRpcToken` 写入公开网页、前端仓库或日志。
- 对公网暴露服务端口前，应先确认 fnOS 网络、防火墙和反向代理访问控制。
- JSON-RPC 入口只用于兼容外部添加任务，不应绕过 Web UI 或后端授权目录校验。
