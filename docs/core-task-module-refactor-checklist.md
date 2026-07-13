# 核心任务模块拆分实施清单

> 本清单对应 `docs/current-project-analysis-report.md` 第 4 节。每项仅在定向测试与 `rtk pnpm run verify:pre-commit` 通过后标记完成。

| 序号 | 任务 | 状态 | Commit |
| --- | --- | --- | --- |
| 1 | 整理后端测试文件 | 已完成 | `refactor(tasks): 整理任务服务测试模块` |
| 2 | 抽离任务查询与持久化同步 | 待处理 | `refactor(tasks): 抽离任务查询与持久化同步` |
| 3 | 抽离任务删除与回收站流程 | 待处理 | `refactor(tasks): 抽离任务删除与回收站流程` |
| 4 | 抽离暂停、恢复与重新下载流程 | 待处理 | `refactor(tasks): 抽离任务控制流程` |
| 5 | 抽离普通 URL 与种子创建流程 | 待处理 | `refactor(tasks): 抽离 URL 与种子任务创建流程` |
| 6 | 抽离磁链解析与文件确认流程 | 待处理 | `refactor(tasks): 抽离磁链解析与文件确认流程` |
| 7 | 收口后端服务入口 | 待处理 | `refactor(tasks): 收口任务服务入口` |
| 8 | 抽离前端批量任务操作 | 待处理 | `refactor(tasks): 抽离批量任务操作` |
| 9 | 抽离前端顶部操作状态 | 待处理 | `refactor(tasks): 抽离顶部任务操作状态` |
| 10 | 抽离页面弹窗与启动刷新编排 | 待处理 | `refactor(ui): 收口主窗口页面编排` |
| 11 | 最终回归与文档收口 | 待处理 | `docs(architecture): 完成核心任务模块拆分收口` |

## 验证规则

- 后端任务先运行对应 Rust 定向测试。
- 前端任务先运行对应 Vitest 文件。
- 每个提交前运行 `rtk pnpm run verify:pre-commit`。
- 第 11 项运行 `rtk pnpm run verify` 并复核核心任务路径。
