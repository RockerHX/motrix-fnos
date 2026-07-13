# Motrix FNOS

飞牛 fnOS 下载管理应用。项目版本以 `package.json`、`server/Cargo.toml` 与 `packaging/fnos/manifest.template` 为核心来源，Release tag 使用 `v<version>`。

本仓库的目标是交付一个可在飞牛 fnOS 中安装、启动、停止、升级和卸载的 FPK-first 下载管理应用：

- `FPK` 交付形态
- `Rust server + Axum` 后端主线
- `Vue Web UI + Naive UI + Pinia` 前端主线
- `Aria2 Next sidecar + SQLite` 运行时基础设施
- `HTTP API + SSE` 前后端通信

## 当前状态

当前发布版本以 `package.json` 与 Release tag 为准；发布流程会校验 `package.json`、`server/Cargo.toml` 与 `packaging/fnos/manifest.template` 三个核心版本源一致。

- 已具备 Rust server、Vue Web UI、Aria2 Next sidecar、SQLite 与 FPK 打包主线。
- 当前主线已完成任务管理、设置、诊断日志、应用内国际化、手机端 UI 适配、关于页和新建下载任务增强能力。
- 新建任务支持单 URL、批量 URL、磁力链接、Multipart 种子文件、立即开始 / 添加后暂停，以及分类、连接数、下载限速和代理高级选项。
- 磁力链接支持 metadata 解析后确认真实文件，再开始下载。
- FPK 仍按设备 CPU 架构区分 x86 与 ARM 两个产物。

当前阶段和验收口径见 [`docs/development-plan.md`](docs/development-plan.md)。

## FPK 构建

安装依赖：

```bash
rtk pnpm install
```

构建 FPK：

```bash
rtk pnpm run build:fpk
```

只做预组装检查：

```bash
rtk pnpm run build:fpk:prepare
```

清理本地生成产物：

```bash
rtk pnpm run clean
```

详细构建矩阵、产物位置、交叉构建说明和打包排障见 [`docs/fpk-packaging.md`](docs/fpk-packaging.md)。

## 文档入口

- 长期架构：[`docs/architecture.md`](docs/architecture.md)
- 阶段状态：[`docs/development-plan.md`](docs/development-plan.md)
- 接口契约：[`docs/api-contract.md`](docs/api-contract.md)
- 打包说明：[`docs/fpk-packaging.md`](docs/fpk-packaging.md)
- UI 产品需求：[`docs/design/ui-product-requirements.md`](docs/design/ui-product-requirements.md)
- UI 设计系统：[`docs/design/DESIGN.md`](docs/design/DESIGN.md)
- Stitch 提示词：[`docs/design/stitch-prompts.md`](docs/design/stitch-prompts.md)

## 本地开发说明

Web UI 类型检查与构建：

```bash
rtk pnpm run typecheck
rtk pnpm run build
```

Server 测试：

```bash
rtk cargo test --manifest-path server/Cargo.toml
```
