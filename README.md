# Motrix FNOS

Motrix FNOS 是面向飞牛 fnOS 的下载管理应用，内置 Aria2 Next 下载引擎，以 FPK 形式安装和运行。用户通过同一套 Web UI 在桌面浏览器、手机浏览器或飞牛 App 中管理下载任务。

## 产品能力

- 支持 HTTP / HTTPS、批量 URL、种子文件和磁力链接。
- 磁力链接解析完成后可先确认文件，再开始真实下载。
- 支持开始、暂停、继续、重新下载、删除、回收站和批量操作。
- 支持任务分类、连接数、单任务限速、代理和全局下载设置。
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

现有 `aria2.getGlobalOption` 仅返回受控的授权下载目录子集，不代表支持 Aria2 Explorer 的完整配置管理。需要远程推送任务时，应使用项目文档中声明的 JSON-RPC 地址和对应 Token；需要查看或控制任务时，应使用 Motrix Web UI。

## 支持平台

FPK 按设备 CPU 架构分别发布：

- `x86_64` 设备安装 x86 包。
- `aarch64` / `arm64` 设备安装 ARM 包。

安装包架构必须与设备匹配。应用运行时由 Rust server 托管 Web UI，并统一管理 Aria2 Next 和 SQLite 数据。

## 安装与使用

1. 从 [GitHub Releases](https://github.com/RockerHX/motrix-fnos/releases) 下载与设备架构匹配的 FPK。
2. 在飞牛应用中心安装 FPK，并为应用添加需要使用的读写文件夹授权。
3. 启动 Motrix，打开应用界面后选择授权目录并创建下载任务。

同一 `appname=motrix` 身份下的后续升级默认保留任务、设置和运行数据。旧 `motrix.fnos` 包切换到 `motrix` 属于新应用安装，旧任务、设置和 JSON-RPC Token 不会自动迁移；安装前应停止旧应用，避免 `17080`、`17081` 端口冲突。

启用 FN Connect 后，当前入口可通过 `https://motrix.<account>.fnos.net/` 打开。卸载时只有明确选择“同时删除 Motrix 应用数据”才会清理应用私有数据，用户下载文件不在清理范围内。

FN Connect / 应用子域名依赖 fnOS 登录态，只适合作为管理入口；第三方解析站不会携带该登录态，公网 JSON-RPC 仍需通过 Lucky/Cloudflare 反向代理到回环 `17081`。

## 技术文档

- [架构与职责边界](docs/architecture.md)
- [开发计划与后续事项](docs/future-development-plan.md)
- [HTTP、SSE 与 JSON-RPC 接口](docs/api-contract.md)
- [开发脚本与命令说明](docs/development-scripts.md)
- [FPK 构建、验证与发布](docs/fpk-packaging.md)
- [发布记录](CHANGELOG.md)
