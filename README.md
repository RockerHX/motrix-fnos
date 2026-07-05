# Motrix FNOS

飞牛 fnOS 下载管理应用。当前基础功能已稳定，阶段 9 应用内国际化已完成，正在规划手机端 UI 适配。

本仓库的目标是交付一个可在飞牛 fnOS 中安装、启动、停止、升级和卸载的 FPK-first 下载管理应用：

- `FPK` 交付形态
- `Rust server + Axum` 后端主线
- `Vue Web UI + Naive UI + Pinia` 前端主线
- `Aria2 Next sidecar + SQLite` 运行时基础设施
- `HTTP API + SSE` 前后端通信

## 当前状态

截至 2026-07-05：

- 已建立 `packaging/fnos/` FPK 目录、`manifest`、`cmd/start`、`cmd/stop`、`cmd/status` 和 Web UI 入口配置。
- 已建立 Rust server、Web UI、Aria2 Next sidecar 的统一组装脚本。
- 已可生成 x86 与 ARM 包：`packaging/fnos/dist/motrix.fnos_1.2.0_x86.fpk`、`packaging/fnos/dist/motrix.fnos_1.2.0_arm.fpk`。
- 飞牛实机已验证安装、启动、停止、状态查询、Web UI、HTTP/HTTPS 下载、暂停、继续、删除、设置、日志和 session 恢复可用。
- Web UI 已支持侧栏分类、回收站、扩展占位页、设置页、帮助入口、诊断日志和应用内中英文切换。
- 手机端 UI 适配已完成本轮开发，详见 [`docs/mobile-ui-adaptation-plan.md`](docs/mobile-ui-adaptation-plan.md)。

注意：FPK 必须与设备 CPU 架构匹配。x86 包不能安装到 ARM 飞牛设备；OES / A311D 等 ARM 飞牛应安装 ARM 包，否则会提示“应用包不符合系统要求”。

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
packaging/fnos/dist/motrix.fnos_1.2.0_x86.fpk
packaging/fnos/dist/motrix.fnos_1.2.0_arm.fpk
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
packaging/fnos/dist/motrix.fnos_1.2.0_arm.fpk
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

## 当前仓库中哪些内容可复用

以下内容仍保留较高迁移价值：

- Vue 3 + Naive UI 的任务、设置、诊断界面结构
- Pinia 状态管理与任务运行态管理模式
- Rust 中的下载任务模型、Aria2 管理、SQLite 持久化、日志与 session 恢复逻辑
- Linux x86_64 / ARM64 的 Aria2 Next sidecar 资产

## 文档入口

- 架构边界：[`docs/architecture.md`](docs/architecture.md)
- 阶段计划：[`docs/development-plan.md`](docs/development-plan.md)
- 手机端 UI 适配计划：[`docs/mobile-ui-adaptation-plan.md`](docs/mobile-ui-adaptation-plan.md)
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
