# 阶段 13：UI 重设计开发计划

> 更新时间：2026-07-17
> 本文档只记录阶段 13 的状态、范围、实施门禁和验收口径。已完成内容见 `CHANGELOG.md`。

## 1. 当前状态

阶段 13 仍处于设计稿准备阶段，尚未开始视觉重设计；已获授权先执行视觉零变化的样式文件组织迁移和受控 UnoCSS 开发期试点。

- 桌面暗色单页 PoC 已完成可渲染 revision 和 Naive UI 可实现性评审。
- 当前等待用户在 Stitch 画布确认母版；母版未批准前不生成其他页面，也不修改 UI 代码。
- 本次例外只允许将现有 scoped 样式外置、增加样式架构约束测试，以及在通用 UI 原语上试验构建期 UnoCSS；不得改变视觉、交互、业务流程或后端接口。
- 现有 38 个 Vue scoped style 已全部迁移为同目录 CSS 文件，样式架构约束测试已纳入 `scripts/tests/`；迁移未改变归一化后的构建 CSS 规则。
- UnoCSS 受控试点已通过，当前只保留通用 UI 原语上的静态 utility safelist，不扩大到其他组件。
- 深色与浅色主题需求已经确认，但当前前端仍固定使用深色主题。
- 重设计只调整视觉层级、信息密度、排版、响应式布局和组件表现，不改变现有业务流程与前后端架构。

## 2. 设计输入与范围

唯一设计输入：

- `docs/design/ui-product-requirements.md`：功能、信息架构、状态和体验需求。
- `docs/design/DESIGN.md`：深浅主题、视觉 token 和组件规则。
- `docs/design/stitch-prompts.md`：Stitch 页面输入。
- `docs/ui-redesign-stitch-figma-workflow.md`：设计执行顺序、母版记录和批准门禁。

首批实现范围：

- 桌面、窄桌面和移动端任务列表与空状态。
- 新建任务、设置、任务详情和磁链文件确认。
- About、Help、Diagnostics 和调试日志。
- 深色与浅色主题，以及主题偏好持久化。

不在本阶段实现：搜索、右键菜单、底部状态栏、做种分类、任务详情抽屉及其他候选功能。

## 3. 实施门禁

1. 用户明确批准 Stitch 母版后，才能派生其他页面。
2. 用户明确批准具体 Figma frame 后，才能制定并执行 UI 代码改造计划。
3. 实现继续使用 Vue 3、Naive UI、Pinia、现有 HTTP API 和 SSE，不引入平行业务状态或另一套移动端工程。
4. 主题偏好的接口、数据结构和迁移策略在主题代码实施前按实际需求重新设计，并同步更新 API 契约。
5. 每批实现必须保持现有任务、设置、诊断和 fnOS 运行流程可用。
6. 在母版批准前，除上述视觉零变化样式迁移和受控 UnoCSS 试点外，不得执行 UI 重设计或安装其他运行时依赖。

## 4.1 样式架构迁移验收

- Vue 文件内联 scoped style：38 → 0。
- 同目录组件 CSS 文件：38 个。
- 迁移前后构建 CSS 在归一化 `data-v-*` scope id 后保持一致；scope id 的变化属于 Vue 外部 style 编译的正常结果。
- 基线合并 CSS：43,493 字节，gzip 8,129 字节；迁移批次未引入视觉规则变化。
- `scripts/tests/style-architecture.test.mjs` 防止后续新增内联组件样式。

## 4.2 UnoCSS 试点验收

- 使用开发依赖 UnoCSS 66.7.5、`presetWind3({ preflight: false })`，关闭默认 extractor 和 pipeline，仅 safelist 11 个静态 utility。
- 试点组件为 AppMetricCard、AppMetricGrid、AppDialogActions，保留语义 class 和所有主题/状态/深层覆盖 CSS。
- 共移除 20 条基础布局声明；未使用任意颜色、任意尺寸或 `!important` utility。
- 试点构建 CSS：43,303 字节，gzip 8,139 字节；相对外置迁移基线 43,493 字节、gzip 8,129 字节，gzip 增长 10 字节，低于 1 KiB 门槛。
- 不继续在任务列表、复杂响应式组件或 Naive UI 深层覆盖中扩大 UnoCSS 使用范围。

## 4. 验收口径

- 覆盖 `390x844`、`1024x768` 和 `1440x900` 三个视口。
- 深色、浅色及中英文组合均无横向溢出、文字遮挡或操作区错位。
- Loading、Empty、Error、Disabled、Selected 和 Runtime exiting 状态表达完整。
- 键盘焦点、触控目标、弹窗滚动和移动端安全区符合设计要求。
- SSE 更新不会造成任务项闪烁、抖动或无意义动画。
- `pnpm run verify:pre-commit` 通过；发布前执行 `pnpm run verify` 和目标 fnOS 实机回归。

## 5. 文档关系

- `docs/architecture.md`：长期架构与职责边界。
- `docs/api-contract.md`：HTTP、SSE 与 JSON-RPC 接口契约。
- `docs/fpk-packaging.md`：FPK 构建、发布和实机验证。
- `CHANGELOG.md`：已完成功能与发布历史。

## 6. 后期平台实验：统一网关可信性验证

状态：延期、非阻塞，不属于阶段 13。当前 Motrix 继续使用已经实机验证的端口入口；本实验不得阻塞功能开发、常规修复或正式发布。

实验必须使用独立 appname 的最小 FPK，不得直接修改 `motrix.fnos` 主包。最小包只包含：

- 一个 `gatewayPrefix` 与一个位于 `TRIM_APPDEST` 的 Unix Socket。
- 一个返回固定 HTML 的根路由和一个返回固定 JSON 的健康检查路由。
- 一个 SSE 或 WebSocket 回显路由，用于验证长连接转发。
- 记录请求 path、`X-Trim-Userid`、`X-Trim-Isadmin` 和 `X-Trim-Username` 是否存在的脱敏日志。

实机验证矩阵：

1. 全新安装后，确认 `trim_sac/open_gateway` 生成入口和转发表，登录用户访问公开路径返回 `200`。
2. 直接请求 Unix Socket 与通过 fnOS nginx 请求得到一致响应，并记录网关转发时 path 是保留还是剥离前缀。
3. 未登录请求被 fnOS 拒绝；已登录请求携带可信 `X-Trim-*` Header。
4. 根 HTML、子路径、静态资源、健康检查及 SSE/WebSocket 均能通过同一网关前缀访问。
5. 分别验证停止后启动、覆盖升级、卸载重装和设备重启，确认 Socket 清理与路由注册不会残留或丢失。
6. 收集 fnOS 版本、入口数据库记录、nginx access/error log、注册服务日志和最小 FPK 校验和，形成可复现报告。

迁移门禁：

- 最小 FPK 在目标 fnOS 实机上完整通过上述矩阵后，才能提出 Motrix 统一网关迁移方案。
- 迁移前必须先更新 `docs/architecture.md`、`docs/api-contract.md` 和 `docs/fpk-packaging.md`，并明确端口模式回退方案。
- Motrix 的候选实现必须单独验证 FPK 最终产物和真实 fnOS 转发链路；仅有 Axum Router 测试或 Unix Socket 直连测试不算通过。
- 任一注册、鉴权、路径或长连接场景失败时，Motrix 继续保持当前端口入口，不合并实验代码。
