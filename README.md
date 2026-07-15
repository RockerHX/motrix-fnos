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

## 支持平台

FPK 按设备 CPU 架构分别发布：

- `x86_64` 设备安装 x86 包。
- `aarch64` / `arm64` 设备安装 ARM 包。

安装包架构必须与设备匹配。应用运行时由 Rust server 托管 Web UI，并统一管理 Aria2 Next 和 SQLite 数据。

## 安装与使用

1. 从 [GitHub Releases](https://github.com/RockerHX/motrix-fnos/releases) 下载与设备架构匹配的 FPK。
2. 在飞牛应用中心安装 FPK，并为应用添加需要使用的读写文件夹授权。
3. 启动 Motrix，打开应用界面后选择授权目录并创建下载任务。

升级默认保留任务、设置和运行数据。卸载时只有明确选择“同时删除 Motrix 应用数据”才会清理应用私有数据，用户下载文件不在清理范围内。

## 技术文档

- [架构与职责边界](docs/architecture.md)
- [当前开发阶段](docs/development-plan.md)
- [HTTP、SSE 与 JSON-RPC 接口](docs/api-contract.md)
- [开发脚本与命令说明](docs/development-scripts.md)
- [FPK 构建、验证与发布](docs/fpk-packaging.md)
- [发布记录](CHANGELOG.md)
