# fnOS 安装与升级验收记录

状态：待目标 fnOS 实机验证。本文档是证据模板，不代表当前版本已经通过设备验收。

## 基本信息

| 项目 | 记录 |
| --- | --- |
| FPK 版本 | `1.8.1` |
| 设备型号 / CPU | 待填写（x86_64 或 aarch64） |
| fnOS 版本 | 待填写 |
| FPK SHA-256 | 待填写 |
| 验证时间 | 待填写 |
| 验证人 | 待填写 |

## 全新安装

1. 使用与设备架构匹配的 `motrix_<version>_x86.fpk` 或 `motrix_<version>_arm.fpk` 安装。
2. 记录应用中心安装结果、应用身份 `motrix`、管理端口和安装日志。
3. 启动应用，确认 `cmd/status` 报告进程和就绪接口均正常。
4. 首次打开管理页面，完成密码设置、登录和退出。
5. 创建一个小型 URL 任务，确认任务记录、用户文件和 Aria2 session 均生成。

通过标准：安装无报错，管理 listener 可访问，登录和任务创建成功；JSON-RPC 仅在本机回环端口监听。

## 同身份升级

1. 在升级前停止应用，记录任务数量、设置摘要、授权目录、SQLite 文件大小和 Aria2 session SHA-256。
2. 备份 fnOS 实际提供的 `TRIM_PKGVAR` 目录；不要把密码、Session 或 JSON-RPC Token 原文写入记录。
3. 安装同一 `appname=motrix` 身份的新 FPK，启动并等待就绪。
4. 对比任务、设置、授权目录、SQLite 完整性、Aria2 session 和登录状态。
5. 登录后暂停/恢复一个任务，确认升级后仍可读写数据。

通过标准：升级不清空任务、设置、SQLite 或 Aria2 session；服务重新启动且首次请求、登录和任务创建正常。失败时保留升级前备份、`server.log`、`lifecycle.log` 和应用中心日志，不得写成通过。

## 证据清单

- [ ] 安装/升级界面截图或导出日志
- [ ] `appcenter-cli list`、`cmd/status` 输出
- [ ] 升级前后任务和设置摘要
- [ ] SQLite `database-check` 返回码
- [ ] 升级前后 Aria2 session 校验值
- [ ] 管理页面首次登录和任务创建截图
