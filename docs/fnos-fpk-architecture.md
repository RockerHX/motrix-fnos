# FPK-first 目标架构文档

## 目的

沉淀飞牛 fnOS FPK 交付形态下的目标系统架构，作为 `docs/architecture.md` 的专题展开文档。

## 当前状态

阶段 0 仅建立文档骨架，后续在 Rust server、Web UI、打包链路开始实施时补全细节。

## 后续填充范围

- FPK 包结构与目录约定
- Rust server 进程模型与状态管理
- Web UI 部署方式与入口约定
- Aria2 sidecar、SQLite、日志和运行时文件布局
- 与 legacy Tauri 资产的迁移边界

## 与其他文档关系

- 总体边界以 `docs/architecture.md` 为准。
- API 细节见 `docs/api-contract.md`。
- 打包细节见 `docs/fpk-packaging.md`。
