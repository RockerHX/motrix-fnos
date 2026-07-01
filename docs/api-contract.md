# 前后端 HTTP / SSE API 契约

## 目的

定义 Rust server 与 Vue Web UI 之间的 HTTP API、SSE 事件流和错误响应约定。

## 当前状态

阶段 0 仅建立文档骨架，具体接口列表与请求/响应结构将在 HTTP API 阶段开始前补全。

## 后续填充范围

- `/api/*` 路由清单
- 请求参数与响应结构
- 统一错误响应模型
- SSE 事件名、字段与订阅方式
- 兼容策略与版本约定

## 与其他文档关系

- 总体架构边界见 `docs/architecture.md`。
- FPK 交付与部署要求见 `docs/fpk-packaging.md`。
- 实机验证项见 `docs/fnos-manual-test-checklist.md`。
