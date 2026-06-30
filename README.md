# Motrix FNOS

飞牛 OS 专用 GUI 下载工具。当前仓库处于 **阶段 0：工程骨架搭建**，目标是验证 Tauri GUI、Vue 前端、Rust 后端通信，以及 Aria2 Next 进程/RPC 连接能力。

## 技术栈

- Rust + Tauri 2
- Vue 3 + TypeScript + Vite
- pnpm
- Aria2 Next（阶段 0 使用外部路径配置，不提交二进制）

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

## 配置 Aria2 Next

阶段 0 不把 Aria2 Next 二进制放进仓库。需要验证真实引擎启动时，先设置环境变量：

```bash
export MOTRIX_FNOS_ARIA2_PATH=/path/to/aria2c
rtk pnpm tauri:dev
```

应用内“Aria2 Next / 引擎状态验证”区域提供：

- 路径配置检查
- 启动引擎
- 停止引擎
- 检查 RPC（调用 `aria2.getVersion`）

未设置路径或路径无效时，界面应显示明确错误，不应崩溃。

## 阶段 0 完成标准

- 应用窗口标题为 `Motrix FNOS`
- GUI 可显示深色主窗口占位布局
- 前端能调用 Rust 命令并显示应用信息
- Aria2 Next 路径、进程和 RPC 状态可验证
- 不实现真实下载任务 CRUD
- 不引入 Axum
- 不做 FPK 打包
