# Motrix FNOS

飞牛 fnOS 下载管理应用。当前版本为 1.2.1，基础功能、应用内国际化和手机端 UI 适配均已完成本轮开发。

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
- 已可生成 x86 与 ARM 包：`packaging/fnos/dist/motrix.fnos_1.2.1_x86.fpk`、`packaging/fnos/dist/motrix.fnos_1.2.1_arm.fpk`。
- 飞牛实机已验证安装、启动、停止、状态查询、Web UI、HTTP/HTTPS 下载、暂停、继续、删除、设置、日志和 session 恢复可用。
- Web UI 已支持侧栏分类、回收站、扩展占位页、设置页、帮助入口、关于页、诊断日志和应用内中英文切换。
- 手机端 UI 适配已完成本轮开发，覆盖移动端外壳布局、任务卡片和核心弹窗。
- 关于页已支持应用信息、作者、项目链接、版本检测、手动更新说明和更新历史展示；版本更新仍需下载匹配架构 FPK 后在 fnOS 应用中心手动安装，或未来上架后通过应用中心更新。

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
