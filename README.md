# Motrix FNOS

飞牛 fnOS 下载管理应用。当前版本为 1.3.3，基础功能、应用内国际化、手机端 UI 适配和关于页能力均已完成本轮开发。

本仓库的目标是交付一个可在飞牛 fnOS 中安装、启动、停止、升级和卸载的 FPK-first 下载管理应用：

- `FPK` 交付形态
- `Rust server + Axum` 后端主线
- `Vue Web UI + Naive UI + Pinia` 前端主线
- `Aria2 Next sidecar + SQLite` 运行时基础设施
- `HTTP API + SSE` 前后端通信

## 当前状态

当前版本：`1.3.3`

- 已具备 Rust server、Vue Web UI、Aria2 Next sidecar、SQLite 与 FPK 打包主线。
- 当前版本已完成任务管理、设置、诊断日志、应用内国际化、手机端 UI 适配和关于页能力。
- FPK 仍按设备 CPU 架构区分 x86 与 ARM 两个产物。

详细阶段状态、已完成里程碑和验收口径见 [`docs/development-plan.md`](docs/development-plan.md)。

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
- JSON-RPC 远程访问：[`docs/jsonrpc-remote-access.md`](docs/jsonrpc-remote-access.md)
- 打包说明：[`docs/fpk-packaging.md`](docs/fpk-packaging.md)
- 历史 UI 设计归档：[`docs/design/archive/ui-stitch-prompts.md`](docs/design/archive/ui-stitch-prompts.md)

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
