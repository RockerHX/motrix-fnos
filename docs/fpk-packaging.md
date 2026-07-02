# FPK 打包与交付说明

## 目的

记录飞牛 fnOS 下的 FPK 包结构、`fnpack` 使用方式、构建输入和安装调试流程。

## 当前状态

阶段 4 已启动，当前已明确 Linux x86_64 server 的标准构建入口与产物位置：

- 构建命令：`pnpm run build:server:linux:x64`
- 默认目标：`x86_64-unknown-linux-gnu`
- 产物路径：`server/target/x86_64-unknown-linux-gnu/release/motrix-fnos-server`
- 非 Linux x86_64 主机默认通过 `cargo-zigbuild` 执行交叉构建；Linux x86_64 主机可直接使用 `cargo build`。

## 后续填充范围

- `packaging/fnos/` 目录结构
- manifest、config、cmd 脚本约定
- Rust server 与 Web UI 构建产物放置方式
- Web UI 构建命令：`pnpm run build:web:fpk`，同步输出到 `packaging/fnos/ui/dist/`。
- Aria2 sidecar 集成方式
- `fnpack build`、安装、调试和排障流程

## 与其他文档关系

- 总体架构边界见 `docs/architecture.md`。
- 目标架构专题见 `docs/fnos-fpk-architecture.md`。
- 实机验证步骤见 `docs/fnos-manual-test-checklist.md`。
