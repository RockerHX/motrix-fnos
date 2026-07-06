# Motrix FNOS

飞牛 fnOS 下载管理应用。当前版本为 1.2.1，基础功能、应用内国际化和手机端 UI 适配均已完成本轮开发。

本仓库的目标是交付一个可在飞牛 fnOS 中安装、启动、停止、升级和卸载的 FPK-first 下载管理应用：

- `FPK` 交付形态
- `Rust server + Axum` 后端主线
- `Vue Web UI + Naive UI + Pinia` 前端主线
- `Aria2 Next sidecar + SQLite` 运行时基础设施
- `HTTP API + SSE` 前后端通信

## 当前状态

当前版本：`1.2.1`

- 已具备 Rust server、Vue Web UI、Aria2 Next sidecar、SQLite 与 FPK 打包主线。
- 当前版本已完成任务管理、设置、诊断日志、应用内国际化、手机端 UI 适配和关于页能力。
- FPK 仍按设备 CPU 架构区分 x86 与 ARM 两个产物。

详细阶段状态、已完成里程碑和验收口径见 [`docs/development-plan.md`](docs/development-plan.md)。

## FPK 构建

安装依赖：

```bash
rtk pnpm install
```

同时构建 x86 和 ARM 包：

```bash
rtk pnpm run build:fpk
```

输出：

```text
packaging/fnos/dist/motrix.fnos_1.2.1_x86.fpk
packaging/fnos/dist/motrix.fnos_1.2.1_arm.fpk
```

如需只构建 x86 包：

```bash
rtk pnpm run build:fpk:x64
```

如需只构建 ARM64 / aarch64 包：

```bash
rtk pnpm run build:fpk:arm64
```

输出：

```text
packaging/fnos/dist/motrix.fnos_1.2.1_arm.fpk
```

非 Linux x86_64 主机进行交叉构建时，脚本会自动检查并安装缺失的 Rust target；仍需要先安装 `cargo-zigbuild` / `ziglang`：

```bash
rtk cargo install --locked cargo-zigbuild
rtk python3 -m pip install --user --break-system-packages ziglang
```

如果只想验证 FPK 组装目录，不执行 `fnpack build`：

```bash
rtk pnpm run build:fpk:prepare
```

清理本地生成产物：

```bash
rtk pnpm run clean
```

该命令只清理源码态仓库不应长期保留的构建输出和系统残留，例如 `dist/`、`packaging/fnos/app/bin/` 中 staged 的二进制、`packaging/fnos/app/ui/dist/`、`packaging/fnos/dist/`、`packaging/fnos/motrix.fnos.fpk` 和 `.DS_Store`。`assets/aria2/` 下的 Aria2 Next Linux sidecar 是当前打包脚本的源资产，不属于可清理产物。

## 当前仓库中哪些内容可复用

以下内容仍保留较高迁移价值：

- Vue 3 + Naive UI 的任务、设置、诊断界面结构
- Pinia 状态管理与任务运行态管理模式
- Rust 中的下载任务模型、Aria2 管理、SQLite 持久化、日志与 session 恢复逻辑
- Linux x86_64 / ARM64 的 Aria2 Next sidecar 资产

## 文档入口

- 架构边界：[`docs/architecture.md`](docs/architecture.md)
- 阶段计划：[`docs/development-plan.md`](docs/development-plan.md)
- HTTP / SSE 契约：[`docs/api-contract.md`](docs/api-contract.md)
- FPK 打包说明：[`docs/fpk-packaging.md`](docs/fpk-packaging.md)

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
