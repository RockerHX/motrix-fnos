# Motrix fnOS UI 重设计启动与协作流程

> 用途：在新 Codex 对话中启动“需求文档 + 设计系统 + Stitch/Figma 关键页面 + 分阶段实现”工作流。  
> 当前阶段：仅定义流程和提示词，不代表视觉方案已经确认。  
> 技术边界：继续使用 Vue 3、TypeScript、Naive UI、Pinia，不改变 `docs/architecture.md` 规定的业务边界。

## 1. 已确认的重设计决策

确认日期：2026-07-12  
确认结果：`1A 2B 3B 4B 5A`

以下五项已经用户确认。后续设计文档、Stitch 提示词、设计稿评审和代码实施必须遵守这些选择；在新设计文档获得用户确认前，不得修改 UI 代码或删除旧文档。

### 1.1 重设计幅度

- [x] **A（已确认）**：保留当前信息架构和业务流程，系统性重做视觉层级、密度、排版和组件表现。
- [ ] B：允许重新设计侧栏、工具栏和任务列表结构，但每项结构变化必须先评估业务与实现成本。

### 1.2 主题范围

- [ ] **A（推荐）**：本轮只设计深色主题。
- [x] **B（已确认）**：同时设计深色和浅色主题，并同步增加主题切换需求、token 和测试范围。

### 1.3 第一批 Stitch 页面

- [ ] **A（推荐）**：桌面任务列表、桌面空状态、移动任务列表、新建任务弹窗、设置弹窗、任务详情/文件确认状态。
- [x] **B（已确认）**：在 A 的基础上同时设计 About、Help、Diagnostics。

### 1.4 功能边界

- [ ] **A（推荐）**：严格按当前已实现功能设计，不加入搜索、右键菜单、底部状态栏、任务详情抽屉等未实现能力。
- [x] **B（已确认）**：允许设计稿提出新功能，但必须单独列为候选需求，不得混入首轮 UI 重构实现。

### 1.5 视觉基调

- [x] **A（已确认）**：fnOS 原生工具感，深炭灰、克制绿色、高信息密度、低动效。
- [ ] B：更接近新版 Motrix，品牌表现更强，允许更多卡片和动效。
- [ ] C：更接近 Linear，冷静、轻边框、极简高密度。

上述确认结果还必须同步写入后续创建的产品需求、设计系统和 Stitch 提示词文档，不得只保留在本文件或对话中。

## 2. Skill 使用顺序

已安装的三个 Skill 属于同一仓库，但职责不同，按阶段调用，不要在一个提示词中要求三者同时主导。

1. `redesign-existing-projects`：审计当前实现、功能状态和视觉问题，形成需求与问题清单。
2. `stitch-design-taste`：把确认后的产品约束转成 `DESIGN.md` 和 Google Stitch 提示词。
3. `design-taste-frontend`：Figma 方案确认后指导代码实现和视觉验收。

Skill 的建议低于仓库规则优先级。遇到以下冲突时，以本项目约束为准：

- 本项目是高频下载管理工具，不是营销网站，不设计 Hero、宣传区或滚动叙事。
- 不为了“破格”而破坏高密度信息扫描、桌面原生感或移动端可操作性。
- UI 基础控件优先复用 Naive UI，业务组件遵守 `features/*` 边界。
- 不默认新增 GSAP、Motion、字体包、图标库或其他前端依赖。
- 不采用过度动画、玻璃拟态、装饰渐变、嵌套卡片或大圆角 SaaS 风格。

## 3. Codex 可自动完成的工作

### 阶段 A：现状审计与需求整理

Codex 自动执行：

1. 阅读 `docs/architecture.md`、`docs/api-contract.md`、`docs/development-plan.md` 和当前 Vue 源码。
2. 运行应用和相关测试可行时，核对桌面、窄桌面和移动端的真实状态。
3. 只根据已实现代码整理页面、弹窗、操作、加载、空、错误、禁用和响应式状态。
4. 输出 `docs/design/ui-product-requirements.md`，记录用户、核心任务、信息架构、页面状态、功能边界和验收条件。
5. 明确指出旧 Stitch 归档里已经失真或尚未实现的内容，不将其自动升级为产品需求。

### 阶段 B：设计系统和 Stitch 输入

在用户确认第 1 节决策后，Codex 自动执行：

1. 输出 `docs/design/DESIGN.md`，定义产品氛围、颜色 token、排版、间距、圆角、边框、阴影、状态色、密度、动效和响应式规则。
2. 输出 `docs/design/stitch-prompts.md`，包含全局上下文、每个关键页面提示词、负面约束和真实示例数据。
3. 每个页面提示词同时描述桌面/移动行为以及空、加载、错误、禁用、选中等必要状态。
4. 检查提示词不包含当前产品没有的功能；候选新功能必须放到独立的“未来评估”章节。

### 阶段 C：设计稿评审

用户提供 Stitch 截图或 Figma 设计后，Codex 自动执行：

1. 对照需求文档检查页面完整性、信息层级、交互状态、响应式和可访问性。
2. 检查是否可由现有 Naive UI、Vue 组件和 CSS token 稳定实现。
3. 输出逐页面评审结论：接受、需要调整、拒绝，并给出可直接回填 Stitch 的修订提示词。
4. 用户最终确认后，生成分阶段代码实施计划、测试计划和提交拆分。

### 阶段 D：代码实现

用户明确要求实现后，Codex 使用 `design-taste-frontend`，按阶段修改代码：

1. 先落地全局 token、Shell、导航和任务列表，不一次性重写全部页面。
2. 再迁移创建任务、文件确认、设置、诊断、帮助和关于等弹窗。
3. 保留 Pinia、service、HTTP/SSE 和后端接口，不让设计稿生成第二套业务状态。
4. 每阶段运行窄测试、全量单测、类型检查和响应式视觉验证，并独立提交。

## 4. 用户需要手动完成的流程

当前工作区没有 Stitch MCP 或 Figma MCP，以下步骤由用户完成。

### 4.1 Google Stitch

1. 打开 <https://labs.google/stitch> 并创建项目 `Motrix fnOS UI Redesign`。
2. 先提交 `docs/design/stitch-prompts.md` 中的“全局上下文”，建立统一视觉方向。
3. 按页面逐个提交提示词；每次只生成一个页面或一个状态，不一次生成整个应用。
4. 每个页面生成 2–3 个候选版本，保留相同功能数据，避免候选稿因为内容不同而无法比较。
5. 将候选稿截图发给 Codex 评审；截图应包含完整画布和明确的 viewport 尺寸。
6. 根据 Codex 返回的修订提示词继续迭代，直到页面被标记为“可进入 Figma”。

### 4.2 Figma

1. 将选中的 Stitch 页面导入或重建到同一个 Figma 文件。
2. 建立 Desktop、Mobile、Components、Tokens 四个页面，避免所有 frame 混在一个画布。
3. 将重复元素设为组件及 variant，至少覆盖导航项、工具栏按钮、任务卡片、状态标签、空状态、弹窗和表单控件。
4. 为关键 frame 标明尺寸；建议桌面至少包含 `1440×900` 和 `1024×768`，移动端至少包含 `390×844`。
5. 补齐 hover、active、disabled、loading、error、empty、selected 等 Stitch 容易遗漏的状态。
6. 开启可查看或 Dev Mode 权限，将 Figma 链接、目标页面/节点名称发给 Codex；若无法授权，则导出 2x PNG 并附颜色、字体、间距和尺寸标注。
7. 最终由用户明确回复哪些 frame 已批准，未批准稿不得进入代码实现。

## 5. 文档迁移与清理规则

### 5.1 新增文档

- `docs/design/ui-product-requirements.md`：当前功能与体验需求的唯一来源。
- `docs/design/DESIGN.md`：当前视觉系统和设计 token 的唯一来源。
- `docs/design/stitch-prompts.md`：当前 Stitch 输入和迭代记录。

### 5.2 需要删除的旧文档

当上述三份新文档经用户确认后，删除：

- `docs/design/archive/ui-stitch-prompts.md`

删除原因：该归档包含“全部任务、做种、搜索、右键菜单、底部状态栏、任务详情抽屉”等与当前实现不一致或尚未确认的内容；保留它会产生两个设计来源。

### 5.3 删除旧文档时必须同步更新

以下文件当前直接引用旧归档，删除时必须在同一提交更新：

- `README.md`：将“历史 UI 设计归档”替换为新的产品需求、设计系统和 Stitch 提示词入口。
- `docs/architecture.md`：将顶部历史归档引用替换为当前设计系统引用，但不改变长期架构约束。
- `docs/api-contract.md`：删除与接口契约无关的历史设计归档引用，必要时只链接产品需求文档。
- `docs/development-plan.md`：移除旧归档说明，增加新 UI 重构阶段和状态入口。

### 5.4 不应删除或由设计稿覆盖

- `docs/architecture.md`：继续作为职责边界和技术架构来源。
- `docs/api-contract.md`：继续作为前后端接口来源。
- `docs/development-plan.md`：继续记录实施阶段和完成状态。
- `docs/fpk-packaging.md`、`docs/jsonrpc-remote-access.md`：不属于 UI 设计迁移范围。

## 6. 新对话启动提示词

将下面整段复制到一个已经重新加载 Skill 的 Codex 新对话中：

```text
请使用已安装的 redesign-existing-projects skill 启动 Motrix fnOS UI 重设计前期工作。

开始前必须阅读：
- AGENTS.md
- docs/architecture.md
- docs/ui-redesign-stitch-figma-workflow.md
- docs/api-contract.md
- docs/development-plan.md
- 当前 src/ 前端源码

本项目是 fnOS 上的下载管理工具，技术栈保持 Vue 3、TypeScript、Naive UI、Pinia。它不是营销网站，不允许为了设计稿改变业务架构、复制业务状态或加入无法落地的静态假功能。

第一步只做只读审计，并让我确认 docs/ui-redesign-stitch-figma-workflow.md 第 1 节的五项决策。能够从源码确认的事实不要询问我。

确认后自动完成：
1. 创建 docs/design/ui-product-requirements.md；
2. 使用 stitch-design-taste 的输出结构创建 docs/design/DESIGN.md；
3. 创建 docs/design/stitch-prompts.md，提供全局提示词和第一批关键页面提示词；
4. 标明每项需求来自当前实现、用户确认或未来候选，不得混淆；
5. 审核旧 docs/design/archive/ui-stitch-prompts.md 中失真的需求。

新文档完成并通过测试/一致性检查后，先向我汇报差异并等待确认。只有我确认新文档可作为唯一设计来源后，才允许：
- 删除 docs/design/archive/ui-stitch-prompts.md；
- 同步更新 README.md、docs/architecture.md、docs/api-contract.md、docs/development-plan.md 中的引用。

前期不得修改 UI 代码，不得安装新的运行时依赖，不得直接开始视觉重构。每次文档修改都要保持与真实源码和架构一致。
```

## 7. Stitch 全局提示词模板

本模板只是新对话生成正式 `docs/design/stitch-prompts.md` 时的输入骨架，最终色值和尺寸必须来自用户确认后的 `DESIGN.md`。

```text
Design a production-grade download manager for fnOS named Motrix. This is a frequently used desktop utility, not a marketing website and not a generic SaaS dashboard.

Preserve the approved information architecture and only represent features explicitly listed in the supplied product requirements. Do not invent search, context menus, bottom status bars, seeding categories, cloud accounts, collaboration, analytics, or system controls that are not in the requirements.

The interface should feel native to fnOS: calm, reliable, compact, and optimized for repeated scanning and task operations. Use a dark charcoal neutral system with one restrained green accent. Keep contrast accessible, typography clear for Chinese and English, numerical data tabular, borders subtle, shadows rare, and motion minimal.

Prioritize task name, status, progress, downloaded size, speed, ETA, errors, and available actions. Use cards only when they communicate a real item boundary. Avoid nested cards, oversized headings, decorative gradients, glassmorphism, floating ornamental shapes, marketing composition, and excessive rounded pills.

Use familiar icons, stable control dimensions, clear hover/focus/pressed/disabled/loading states, and responsive layouts designed separately for desktop and mobile. All text must fit without overlap. The final design must be implementable with Vue 3 and Naive UI rather than relying on custom canvas effects or nonstandard controls.

Follow the supplied DESIGN.md for exact colors, spacing, typography, radius, density, responsive breakpoints, and component behavior.
```

## 8. 验收条件

前期设计文档阶段完成的条件：

- 五项产品决策有明确记录。
- 新需求文档与当前源码功能一致。
- `DESIGN.md` 不包含营销站或高动效规则。
- Stitch 提示词覆盖核心页面及关键状态，并有明确负面约束。
- 旧归档中的虚构/过时功能已被识别。
- 用户确认新文档后，旧归档及四处引用被一次性清理。
- 在 Figma frame 获得明确批准前，不开始代码重构。
