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

- `packaging/fnos/` 目录结构（当前已补齐 `manifest`、`config/resource`、`config/privilege`、`cmd/main`、`app/ui/config`）
- manifest、config、cmd 脚本约定
- 启动脚本：`packaging/fnos/cmd/start`，优先使用 `TRIM_APPDEST`、`TRIM_PKGVAR`、`TRIM_SERVICE_PORT`，本地回退到仓库内相对路径；本机验证可通过 `MOTRIX_FNOS_SERVER_BIN` / `MOTRIX_FNOS_ARIA2_PATH` 覆写到 native 二进制。
- 停止脚本：`packaging/fnos/cmd/stop`，通过 `SIGINT` 触发 server 统一退出流程，并等待最多 20 秒完成收口。
- 状态脚本：`packaging/fnos/cmd/status`，运行中返回 0，未运行返回 1，并输出 PID 与监听地址。
- manifest 当前默认产出 x86 包，Web 入口键名为 `motrix.fnos.main`，后续打包脚本可按目标架构改写 `platform` 与 `port` 字段。
- 一键打包脚本：`node scripts/build-fpk.mjs [--target <triple>] [--prepare-only]`，默认生成 x86 FPK，`--prepare-only` 仅验证组装流程不执行 `fnpack build`。
- 统一入口：本地使用 `pnpm run build:fpk:prepare` / `pnpm run build:fpk`，CI 复用同一 `build-fpk` 脚本做预组装验证。
- Rust server 与 Web UI 构建产物放置方式
- Web UI 构建命令：`pnpm run build:web:fpk`，同步输出到 `packaging/fnos/ui/dist/`。
- Aria2 sidecar 集成方式
- Aria2 sidecar 放置命令：`pnpm run stage:aria2:x64` / `pnpm run stage:aria2:arm64`，统一输出到 `packaging/fnos/app/bin/aria2-next`。
- `fnpack build`、安装、调试和排障流程

## 与其他文档关系

- 总体架构边界见 `docs/architecture.md`。
- 目标架构专题见 `docs/fnos-fpk-architecture.md`。
- 实机验证步骤见 `docs/fnos-manual-test-checklist.md`。
