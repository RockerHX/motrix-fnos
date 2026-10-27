# 项目级 Codex 指令

全局规则见 `~/.codex/AGENTS.md`；本文件集中维护项目开发执行规则。

## 开始工作前

- 非简单问答、代码或文档修改、架构讨论前，先阅读 [架构说明](docs/architecture.md)。
- 直接指令优先；长期技术决策以架构为准，阶段状态和优先级以 [开发计划](docs/future-development-plan.md) 为准。
- 架构与实际演进不匹配时，先说明并更新架构，再继续实现。
- 开发、发布和排障步骤按需查阅 [维护手册](docs/maintenance.md)，UI 修改查阅 [UI 规范](docs/design/DESIGN.md)。

## 实现与文档

- 新增前端交互进入 `features/*`，入口页面只编排；新增通信默认使用 HTTP API / SSE。
- 组件 scoped 样式放在同目录 `Component.css`，Vue 只保留 `<style scoped src="./Component.css"></style>`；迁移不改变选择器、声明或视觉表现。
- `src/styles/` 只放全局 token、重置、弹窗公共样式和移动基线，不集中存放业务组件样式。
- UnoCSS 只允许已验证的静态 utility safelist；不启用全局 preflight/reset、默认 extractor、attributify、shortcuts 或图标 preset。主题、语义 class、复杂响应式、`:deep()`、伪元素和状态样式继续用外部 scoped CSS。
- 新增长期状态必须考虑 SQLite 持久化路径和迁移策略。
- `docs/` 只保留架构、接口、开发计划、维护手册和 UI 规范；同一规则只在一处完整说明，其他位置链接引用。
- 一次性审计、体积测量、执行报告和验证日志放仓库外，不提交，也不能成为仓库文档的引用依赖。

## 分支、提交与验证

- 分支按 `feature/<版本或功能> -> develop -> main` 流转；独立分支开发，合入 `develop` 集成测试，测试和发布验收通过后才合入 `main`，保持 `main` 可发布。
- 提交使用 Conventional Commits：`type(scope): 中文说明`，跨模块可省略 scope；标题和可选正文使用中文，标识符和专有名词可保留原文。合并、回退提交也须调整为中文说明。
- 每次提交只含当前任务改动；用户要求测试后提交时，先通过相关测试。
- 日常提交做版本一致性、暂存区空白和 Rust 格式等快速检查；分支推送前对该批业务源码执行一次完整测试与构建验证。
- Release 只改发布白名单中的版本文件和 CHANGELOG，视为与已验证业务源码等价，不重复源码测试；出现白名单外改动必须中止。
- Release 只构建并验证新产生的双架构 FPK、SBOM、校验和与构建证明；本地完整打包先完整验证源码，再构建并验收 FPK。
- 依赖安全审计使用独立每周任务，不并入日常推送验证或 Release。
- 命令组合、hooks、发布白名单及副作用统一见 [维护手册](docs/maintenance.md#开发与验证)。

## 测试文件

- 测试实现与业务代码物理分离，不在 `.rs`、`.ts`、`.vue` 业务文件中编写测试函数、夹具或内联 `mod tests { ... }`。
- Rust 模块只保留 `#[cfg(test)] mod tests;`，实现放在对应 `tests.rs` 或 `<module>/tests.rs`；跨模块集成测试放 `server/tests/`。
- 业务 Rust 文件不允许其他用途的 `#[cfg(test)]`，包括测试专用导出、重导出、包装函数、检查方法、mock 或 fixture；共享测试工具只放 `server/src/test_support.rs`。
- 私有实现由模块的测试子模块访问，不为测试扩大生产 API 可见性或添加测试专用接口。
- 前端测试放独立 `*.spec.ts`；构建与发布脚本测试放 `scripts/tests/`。新建测试或调整目录时，须通过该目录中测试物理分离的静态守卫。

## fnOS 平台查证

涉及 fnOS、FPK、应用中心、manifest、`config/resource`、`config/privilege`、`cmd/*`、`TRIM_*`、目录授权、端口入口或安装升级卸载行为时，先查证资料或实机验证，不凭通用 Linux / NAS 经验推断。

资料优先级：

1. [飞牛官方开发文档](https://developer.fnnas.com/docs/guide/)。
2. 可验证第三方 FPK 仓库、社区指南和论坛排障记录。
3. 本仓库解包、实机日志、`fnpack` 行为、安装目录和环境变量实证。
4. 前三者不足时才使用推断，并写明验证步骤。

修改平台行为的实现或文档时，同步记录来源或验证方式；已有平台依据见 [维护手册](docs/maintenance.md#平台依据与未验证边界)。
