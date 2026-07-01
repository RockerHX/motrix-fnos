# Motrix FNOS

飞牛 fnOS 下载管理应用，当前处于 **FPK-first 架构整改期**。

本仓库的目标不是继续完善 Tauri 桌面应用，而是把现有前端和 Rust 业务资产迁移为：

- `FPK` 交付形态
- `Rust server + Axum` 后端主线
- `Vue Web UI + Naive UI + Pinia` 前端主线
- `Aria2 Next sidecar + SQLite` 运行时基础设施

## 当前状态

当前仓库 **还不能直接产出最终 FPK 交付物**。

现阶段正在进行“阶段 0：文档先行，冻结 Tauri 主线”整改，目标是先统一：

- 架构决策来源
- 后续阶段顺序
- 文档与验收口径

在阶段 0 完成前：

- 不新增 Tauri 能力
- 不删除现有 Tauri 主线文件
- 不启动 server/API 迁移实现

## 当前仓库中哪些内容可复用

以下内容仍保留较高迁移价值：

- Vue 3 + Naive UI 的任务、设置、诊断界面结构
- Pinia 状态管理与任务运行态管理模式
- Rust 中的下载任务模型、Aria2 管理、SQLite 持久化、日志与 session 恢复逻辑
- Linux x86_64 / ARM64 的 Aria2 Next sidecar 资产

## Legacy 说明

当前仓库仍保留 `src-tauri/`、Tauri 脚本和 `@tauri-apps/*` 依赖。这些内容仅作为 **legacy 迁移来源**：

- 可用于复用业务逻辑和现有资产
- 不再代表最终交付路线
- 不应继续作为主线能力扩展

## 文档入口

- 架构边界：[`docs/architecture.md`](docs/architecture.md)
- 阶段计划：[`docs/development-plan.md`](docs/development-plan.md)
- FPK-first 整改计划：[`docs/fnos-fpk-remediation-plan.md`](docs/fnos-fpk-remediation-plan.md)

## 本地开发说明（Legacy 链路）

以下命令仅用于查看和维护当前 legacy 代码，不代表最终 FPK 交付方式：

```bash
rtk pnpm install
rtk pnpm tauri:dev
rtk pnpm build
rtk cargo test --manifest-path src-tauri/Cargo.toml
```

最终交付链路将切换到：

- Rust server 独立运行
- 前端纯 Web 静态资源构建
- `fnpack` FPK 打包

## 当前阶段完成标准

阶段 0 完成后，仓库需要满足：

- 主文档不再把 Tauri 写成当前交付主线
- 后续阶段全部以 FPK / Rust server / Web UI 为验收方向
- 新的 FPK 架构、API 契约、打包说明和实机测试文档骨架已建立
