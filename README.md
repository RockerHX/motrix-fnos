# Motrix FNOS

飞牛 OS 专用 GUI 下载工具。当前仓库已完成 **阶段 0：工程骨架搭建**，正在进入 **阶段 1：最小可用下载器（MVP）**。阶段 1 的重点是内置 Aria2 Next sidecar，并打通 HTTP / HTTPS 下载闭环。

## 技术栈

- Rust + Tauri 2
- Vue 3 + TypeScript + Vite
- pnpm
- Aria2 Next（阶段 1 默认使用 Tauri sidecar 内置引擎）

## 环境要求

- Node.js
- pnpm
- Rust / Cargo
- 可选：Aria2 Next 可执行文件

## 安装依赖

```bash
rtk pnpm install
```

## 本地开发

```bash
rtk pnpm tauri:dev
```

仅启动前端开发服务器：

```bash
rtk pnpm dev
```

## 构建与检查

```bash
rtk pnpm typecheck
rtk pnpm build
rtk cargo check --manifest-path src-tauri/Cargo.toml
rtk cargo test --manifest-path src-tauri/Cargo.toml
```

## Aria2 Next 集成

阶段 1 默认通过 Tauri sidecar 内置 Aria2 Next，不依赖系统安装的 aria2。开发调试时仍可用环境变量覆盖内置引擎：

```bash
export MOTRIX_FNOS_ARIA2_PATH=/path/to/aria2-next
rtk pnpm tauri:dev
```

首批 sidecar 目标平台：

- `aarch64-apple-darwin`：macOS Apple Silicon / 当前 M1 开发测试
- `x86_64-unknown-linux-gnu`：飞牛 OS x86 64 位
- `aarch64-unknown-linux-gnu`：飞牛 OS ARM64

当前暂不支持 32 位 Linux。

应用内“Aria2 Next / 引擎状态验证”区域提供：

- 路径配置检查
- 启动引擎
- 停止引擎
- 检查 RPC（调用 `aria2.getVersion`）

内置 sidecar 缺失、环境变量路径无效或 RPC 连接失败时，界面应显示明确错误，不应崩溃。

运行时生命周期、后台驻留、退出清理和 Aria2 端口兜底策略见 [`docs/runtime-lifecycle-and-aria2-strategy.md`](docs/runtime-lifecycle-and-aria2-strategy.md)。核心原则：关闭窗口只隐藏；明确退出才暂停任务、保存状态并停止本应用管理的 sidecar；Aria2 RPC 端口不得硬编码。

## 阶段 0 完成标准（已完成）

- 应用窗口标题为 `Motrix FNOS`
- GUI 可显示深色主窗口占位布局
- 前端能调用 Rust 命令并显示应用信息
- Aria2 Next 路径、进程和 RPC 状态可验证
- 不实现真实下载任务 CRUD
- 不引入 Axum
- 不做 FPK 打包
