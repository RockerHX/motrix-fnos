# Motrix FNOS

Motrix FNOS 是面向飞牛 fnOS 的下载管理应用，内置 Aria2 Next 下载引擎，以 FPK 形式安装和运行。用户通过同一套 Web UI 在桌面浏览器、手机浏览器或飞牛 App 中管理下载任务。

项目友链：[LINUX DO 社区](https://linux.do/)

## 产品能力

- 支持 HTTP / HTTPS、批量 URL、种子文件和磁力链接。
- 磁力链接解析完成后可先确认文件，再开始真实下载。
- 支持开始、暂停、继续、重新下载、删除、回收站和批量操作。
- 支持任务分类、HTTP 连接上限、单任务限速和全局设置；HTTP 使用任务代理开关，BT 使用全局代理开关。
- 下载目录使用 fnOS 授权目录，任务、设置和 Aria2 session 持久化保存。
- 提供简体中文与英文界面，并适配桌面、移动浏览器和飞牛 App WebView。
- 提供 Aria2 状态、诊断日志、版本检测和带 token 的 JSON-RPC 远程添加任务入口。

## 远程推送定位与兼容边界

Motrix FNOS 的远程能力主要用于把下载任务直接推送到家中的 NAS：网盘解析服务、浏览器工具或其他受信任客户端提交下载 URL 后，由 NAS 上的 Motrix/Aria2 Next 负责下载。远程 JSON-RPC 入口是受鉴权和授权目录限制的任务接收接口，不是完整的 Aria2 管理服务。

推荐使用 Motrix Web UI 查看任务、查看进度和执行任务控制。JSON-RPC 兼容层只维护项目明确声明的受控方法；它不会透传任意 Aria2 RPC，也不会暴露 Aria2 内部 secret、代理凭据或完整配置。

本项目不支持将 [Aria2 Explorer](https://github.com/alexhua/Aria2-Explorer) 或其内置 AriaNg 作为 Motrix FNOS 的完整管理客户端。以下能力不属于项目承诺范围：

- 完整 Aria2 任务、文件、Peer 和 Session 查询；
- 通过外部 RPC 读取或修改完整全局配置、任务配置；
- 以 Aria2 Explorer 的连接状态、配置同步或 AriaNg 页面作为兼容性验收标准。

因此，使用 Aria2 Explorer 时因调用未实现方法而出现 `Method not found`、配置同步失败或状态显示异常，属于超出项目支持范围的预期结果，不作为 Motrix FNOS 的兼容缺陷。

现有 `aria2.getGlobalOption` 仅返回受控的授权下载目录子集，不代表支持 Aria2 Explorer 的完整配置管理。

## Issue 与支持范围

本项目由个人维护，无法提供保姆式的一对一技术支持，也不承诺固定响应时效。

公开 Issue 仅用于：

- 报告可复现的 Bug；
- 提交明确的功能建议；

安全问题请按照 [SECURITY.md](SECURITY.md) 的说明，通过私密渠道报告，不要在公开 Issue 中披露漏洞细节。

请优先使用对应的 Issue 模板：

- [提交 Bug 报告](https://github.com/RockerHX/motrix-fnos/issues/new?template=bug_report.yml)；
- [提交功能建议](https://github.com/RockerHX/motrix-fnos/issues/new?template=feature_request.yml)。

提交 Issue 前，请先确认：

- 使用的是最新版本；
- 已阅读相关文档；
- 已填写版本、fnOS 环境、复现步骤和必要日志；
- 已确认不是重复问题。

缺少关键信息、无法复现、与项目无关或重复的问题，可能会被直接关闭。使用咨询请尽量提供完整上下文，维护者会根据时间和问题价值处理。

请尊重开源项目维护者和其他参与者。辱骂、嘲讽、人身攻击、重复纠缠或恶意灌水将直接关闭，必要时锁定 Issue 或限制后续互动。

如果项目对你有帮助，欢迎点 Star、提交修复或反馈清晰的 Bug；Star 是对项目的自愿支持，不是获取技术支持的前置条件。

## 支持平台

FPK 按设备 CPU 架构分别发布：

- `x86_64` 设备安装 x86 包。
- `aarch64` / `arm64` 设备安装 ARM 包。

安装包架构必须与设备匹配。应用运行时由 Rust server 托管 Web UI，并统一管理 Aria2 Next 和 SQLite 数据。

## 安装与使用

1. 从 [GitHub Releases](https://github.com/RockerHX/motrix-fnos/releases) 下载与设备架构匹配的 FPK。
2. 在飞牛应用中心安装 FPK，首次安装在向导中设置并确认管理密码；保留已设密码的应用数据后重装时，两项留空即可。为应用添加需要使用的读写文件夹授权。
3. 启动 Motrix，打开应用界面后选择授权目录并创建下载任务。

管理页面使用安装时设置的密码登录，保留数据重装或升级时沿用原密码。忘记密码时先停止应用，再通过 NAS SSH 终端运行 `sudo -u motrix_fnos /var/apps/motrix/cmd/reset-web-auth`，按提示隐藏输入并确认新密码，成功后启动应用；下载任务、设置、RPC Token 和下载文件均保留。

同一 `appname=motrix` 身份下升级保留数据，但 **2.x 不导入、不展示、不继续或恢复 1.x 任务，首次跨代升级后任务列表重新开始**。旧任务记录及下载文件保留原处，密码、设置、目录授权和 RPC Token 沿用；旧任务及旧操作未完成也不会阻止升级。升级向导需主动同意兼容性与备份说明。需要旧任务进度时先在旧版完成下载，包括回收站中需要恢复的任务。

升级不自动备份或迁移旧任务。升级前停服并完整备份私有数据，外部下载文件另行保护；回滚须恢复配套旧 FPK 和完整备份，不能让旧版直接读取新版数据。步骤见[备份与回滚](docs/maintenance.md#备份与回滚)。

当前开发目标为 2.0.0，候选待真机验收，尚未正式发布；内置及外部引擎仅支持已验证的 Aria2 Next 2.8.6，其他引擎进入受限管理，旧数据保留且不接管旧任务，详见[支持边界](docs/api-contract.md#42-应用信息)。

旧 `motrix.fnos` 包切换到 `motrix` 属于新应用安装，旧任务、设置和 JSON-RPC Token 不会自动迁移；安装前应停止旧应用，避免 `17080`、`17081`、`17082` 端口冲突。

启用 FN Connect 后，当前入口可通过 `https://motrix.<account>.fnos.net/` 打开。卸载时只有明确选择“同时删除 Motrix 应用数据”才会清理应用私有数据，用户下载文件不在清理范围内。

FN Connect / 应用子域名依赖 fnOS 登录态，只适合作为管理入口；第三方解析站不会携带该登录态，公网 JSON-RPC 仍需通过 Lucky/Cloudflare 反向代理到回环 `17081`。

## 使用帮助

- [遇到问题：登录、启停、下载与日志排查](docs/maintenance.md#故障排查)
- [升级前备份与回滚](docs/maintenance.md#备份与回滚)
- [远程推送：JSON-RPC 接入](docs/maintenance.md#远程接入)
- [发布记录](CHANGELOG.md)

## 开发者参考

以下命令和技术文档供开发维护或编写第三方客户端时查阅。日常使用请看上面的“安装与使用”和“使用帮助”。

### 常用开发命令

在仓库根目录执行；工具链版本和完整操作见[维护手册](docs/maintenance.md#开发与验证)。

```bash
pnpm install                  # 安装依赖
pnpm run dev                 # 只启动前端，后端需另行启动
pnpm run verify:pre-commit   # 提交前快速检查
pnpm run verify             # 完整源码测试与构建
pnpm run build:fpk          # 完整验证、双架构打包与解包验收
```

### 技术文档

- [架构与职责边界](docs/architecture.md)
- [开发计划与后续事项](docs/future-development-plan.md)
- [完整接口契约：HTTP、SSE 与 JSON-RPC（开发者参考）](docs/api-contract.md)
- [维护手册：开发、发布、备份回滚与故障排查](docs/maintenance.md)
- [UI 交互与视觉规范](docs/design/DESIGN.md)
