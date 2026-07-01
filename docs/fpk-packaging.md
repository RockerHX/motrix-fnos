# FPK 打包与交付说明

## 目的

记录飞牛 fnOS 下的 FPK 包结构、`fnpack` 使用方式、构建输入和安装调试流程。

## 当前状态

阶段 0 仅建立文档骨架，待 server / Web UI 主线明确后补全具体目录、命令和产物要求。

## 后续填充范围

- `packaging/fnos/` 目录结构
- manifest、config、cmd 脚本约定
- Rust server 与 Web UI 构建产物放置方式
- Aria2 sidecar 集成方式
- `fnpack build`、安装、调试和排障流程

## 与其他文档关系

- 总体架构边界见 `docs/architecture.md`。
- 目标架构专题见 `docs/fnos-fpk-architecture.md`。
- 实机验证步骤见 `docs/fnos-manual-test-checklist.md`。
