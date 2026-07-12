# Motrix fnOS UI 重设计启动与协作流程

## 0. Stitch 调查结论（2026-07-12）

### 0.1 结论

Stitch 当前页面仍标注 **Beta**。本项目确实踩到了流程坑，但不能把此前的不一致全部归因于 Beta：主要问题是把每个页面当成独立的长提示词生成，未先建立并验证项目级设计系统，也没有从现有 Vue 实现反向提取约束。

推荐采用“一个设计系统 + 一个桌面母版 + 派生页面”的流程。先做一个桌面任务列表 PoC，验证颜色、字体、圆角、外壳和二次编辑的一致性，再扩展状态、弹窗、移动端和浅色主题。

### 0.2 来源与证据等级

| 来源 | 已确认内容 | 证据等级 |
| --- | --- | --- |
| [Stitch Docs](https://stitch.withgoogle.com/docs) 与当前产品页面 | Stitch 生成 Web/移动 UI；产品页标注 Beta；网页能力和版本化 UI 需以当前界面复核 | 官方产品/当前页面 |
| [google-labs-code/stitch-skills](https://github.com/google-labs-code/stitch-skills) | 提供 `code-to-design`、`extract-design-md`、`manage-design-system`；要求 `.stitch/DESIGN.md` 的 YAML front matter；通过 MCP 上传、创建、应用设计系统 | Google Labs 公开参考实现；仓库声明非正式支持产品 |
| 本仓库 `docs/architecture.md` 与现有 Vue 源码 | 桌面、移动浏览器、fnOS WebView 共用一套响应式 Web UI；可从现有代码提取约束 | 本项目实证 |
| 社区 Stitch loop / 第三方 MCP | 可作为风险提示或实验材料，不作为主流程依赖 | 未验证 |

`stitch-skills` 不是 Stitch 官方支持产品，因此其脚本、CLI 和 MCP 参数必须在本地环境中逐项验证；不能把它们写成 Stitch 网页 UI 的保证能力。

### 0.3 已解决与未解决的问题

- 已解决：设计系统应是项目级输入；现有 `DESIGN.md` 已补充结构化 YAML token，正文仍是唯一语义规范。
- 已解决：现有代码适合先走 `code-to-design` / `extract-design-md` 思路，再做定向重设计，而不是从零批量生成。
- 已解决：Stitch MCP 已使用 OAuth Bearer token 和 quota project `stitch-502207` 完成只读连接验证；`stitch.googleapis.com` 已启用，可以由 Codex 创建项目、上传 `DESIGN.md`、创建设计系统并读取生成结果。
- 未解决：Beta 的具体配额、模型、导出和回滚行为需以用户账号当前页面为准。
- 未解决：`DESIGN.md` token 是否被 Stitch 正确解析、应用并在二次编辑中保持一致，必须由单页 PoC 验证。

### 0.4 当前门禁

1. 不继续执行旧的 P1-P10 独立自由生成流程。
2. 先用 Web 类型、暗色主题和现有桌面任务页做单页 PoC；最多两个候选。
3. PoC 必须验证：品牌绿、字体层级、侧栏/任务边界/按钮/圆角、二次编辑保持一致、设计系统可派生。
4. PoC 未通过前不生成移动端、浅色主题、设置页或弹窗；通过后才冻结桌面母版。
5. 任何批量自动化前先验证 MCP 权限、项目 ID、设计系统创建结果和一张截图。

> 用途：在新 Codex 对话中启动“需求文档 + 设计系统 + Stitch/Figma 关键页面 + 分阶段实现”工作流。  
> 当前阶段：设计输入已确认，Stitch MCP 已就绪，下一步执行桌面暗色单页 PoC。  
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

### 阶段 C：Stitch PoC 与设计稿评审

Codex 通过 Stitch MCP 自动执行：

1. 创建或复用唯一的私有项目 `Motrix fnOS UI Redesign`，不得改动其他 Stitch 项目。
2. 上传 `docs/design/DESIGN.md`，创建项目级 Design System，并确认项目能列出对应 asset。
3. 使用 `docs/design/stitch-prompts.md` 第 0.3 节生成一张 `DESKTOP` 暗色 Downloading 页面；首轮只生成一个候选。
4. 通过 MCP 读取截图与 HTML，对照需求文档检查页面完整性、信息层级、视觉 token、可访问性和 Naive UI 可实现性。
5. 若只需局部修正，编辑同一 screen；若整体视觉方向失败，最多再生成一个候选。不得进入 P2-P10。
6. 输出 PoC 结论：接受、需要调整或拒绝，并将 project ID、design system asset ID、screen ID、生成参数和结论写入第 9 节。
7. 用户明确批准母版后，才能派生后续页面；Figma frame 最终批准后再生成代码实施计划。

首轮 48 组产物已审计并拒绝进入 Figma：主要问题是跨页面颜色、字体、圆角、外壳和移动导航不一致，且出现 Search、System 主题、Purge、Export Logs 等未批准功能。后续采用“先批准一个桌面基准，再从已批准页面派生”，不再按状态自由生成完整页面。

### 阶段 D：代码实现

用户明确要求实现后，Codex 使用 `design-taste-frontend`，按阶段修改代码：

1. 先落地全局 token、Shell、导航和任务列表，不一次性重写全部页面。
2. 再迁移创建任务、文件确认、设置、诊断、帮助和关于等弹窗。
3. 保留 Pinia、service、HTTP/SSE 和后端接口，不让设计稿生成第二套业务状态。
4. 每阶段运行窄测试、全量单测、类型检查和响应式视觉验证，并独立提交。

## 4. 自动化边界与用户操作

当前工作区已配置 Stitch MCP，但没有 Figma MCP。Stitch 项目创建、设计系统上传、页面生成和产物读取由 Codex 完成；用户只处理账号凭据和设计批准。

### 4.1 Stitch MCP 凭据

Codex 配置使用 `STITCH_ACCESS_TOKEN` 作为 Bearer token，并发送 `X-Goog-User-Project: stitch-502207`。仓库和本文档不得记录 access token。

当 MCP 返回 `Auth required` 或 `Unauthenticated` 时，用户在终端刷新短期 token：

```zsh
NEW_TOKEN="$(gcloud auth application-default print-access-token)"
launchctl setenv STITCH_ACCESS_TOKEN "$NEW_TOKEN"
unset NEW_TOKEN
```

随后完全退出并重新打开 Codex。不要把 token 写入仓库、`.env`、文档、截图或对话。

### 4.2 证据与确定性规则

- Stitch 产品能力优先以 <https://stitch.withgoogle.com/docs> 的对应正文为依据。
- 当前界面行为可以使用用户提供的完整截图作为版本实证。
- 选择 Web 的产品依据来自 `docs/architecture.md` 的单一 Vue Web UI 架构。
- 页面字段和操作边界来自 `docs/design/ui-product-requirements.md`。
- 视觉 token 来自 `docs/design/DESIGN.md`。
- 没有官方正文或当前界面实证的操作，不得写成确定步骤；必须设置截图检查点。

### 4.3 Figma

1. 将选中的 Stitch 页面导入或重建到同一个 Figma 文件。
2. 建立 Desktop、Mobile、Components、Tokens 四个页面，避免所有 frame 混在一个画布。
3. 将重复元素设为组件及 variant，至少覆盖导航项、工具栏按钮、任务卡片、状态标签、空状态、弹窗和表单控件。
4. 为关键 frame 标明尺寸；建议桌面至少包含 `1440×900` 和 `1024×768`，移动端至少包含 `390×844`。
5. 补齐 hover、active、disabled、loading、error、empty、selected 等 Stitch 容易遗漏的状态。
6. 开启可查看或 Dev Mode 权限，将 Figma 链接、目标页面/节点名称发给 Codex；若无法授权，则导出 2x PNG 并附颜色、字体、间距和尺寸标注。
7. 最终由用户明确回复哪些 frame 已批准，未批准稿不得进入代码实现。

## 5. 文档迁移与清理状态

迁移已于 2026-07-12 经用户确认并完成；以下三份文档现为唯一设计来源。

### 5.1 新增文档

- `docs/design/ui-product-requirements.md`：当前功能与体验需求的唯一来源。
- `docs/design/DESIGN.md`：当前视觉系统和设计 token 的唯一来源。
- `docs/design/stitch-prompts.md`：当前 Stitch 输入和迭代记录。

### 5.2 已删除的旧文档

以下旧文档已删除：

- `docs/design/archive/ui-stitch-prompts.md`

删除原因：该归档包含“全部任务、做种、搜索、右键菜单、底部状态栏、任务详情抽屉”等与当前实现不一致或尚未确认的内容；保留它会产生两个设计来源。

### 5.3 已同步更新的引用

以下文件已在删除旧归档的同一提交中更新：

- `README.md`：将“历史 UI 设计归档”替换为新的产品需求、设计系统和 Stitch 提示词入口。
- `docs/architecture.md`：将顶部历史归档引用替换为当前设计系统引用，但不改变长期架构约束。
- `docs/api-contract.md`：删除与接口契约无关的历史设计归档引用，必要时只链接产品需求文档。
- `docs/development-plan.md`：移除旧归档说明，增加新 UI 重构阶段和状态入口。

### 5.4 不应删除或由设计稿覆盖

- `docs/architecture.md`：继续作为职责边界和技术架构来源。
- `docs/api-contract.md`：继续作为前后端接口来源。
- `docs/development-plan.md`：继续记录实施阶段和完成状态。
- `docs/fpk-packaging.md`、`docs/jsonrpc-remote-access.md`：不属于 UI 设计迁移范围。

## 6. 后续对话启动提示词

三份设计文档已经确认。后续新对话直接执行 Stitch MCP 单页 PoC，不得重复调查、重新生成设计来源或依赖本对话记忆。将下面整段复制到新对话中：

```text
请开始 Motrix fnOS UI 重设计的 Stitch MCP 单页 PoC。

开始前必须阅读：
- AGENTS.md
- docs/architecture.md
- docs/ui-redesign-stitch-figma-workflow.md
- docs/design/ui-product-requirements.md
- docs/design/DESIGN.md
- docs/design/stitch-prompts.md

上述三份 docs/design 文档已经用户确认，是当前唯一设计来源，无需重新确认或重新生成。旧 Stitch 归档已删除。Stitch MCP 已完成只读连接验证，Google Cloud quota project 为 stitch-502207，stitch.googleapis.com 已启用。

我明确授权本次任务在 Stitch 中创建一个私有项目 `Motrix fnOS UI Redesign`，上传 docs/design/DESIGN.md，创建项目级 Design System，并生成一张 DESKTOP 暗色 Downloading PoC。先检查是否已有同名项目，禁止改动或删除其他项目；若同名项目存在则先读取并判断是否属于本流程，禁止盲目重复创建。

严格执行 docs/ui-redesign-stitch-figma-workflow.md 阶段 C：首轮只生成一个候选，通过 MCP 读取截图和 HTML并评审。仅在整体方向失败时才生成第二个候选；不得继续 P2-P10、移动端、浅色主题、弹窗或 Figma。把 project ID、design system asset ID、screen ID、生成参数和评审结论更新到 workflow 第 9 节。

在我明确批准母版和后续 Figma frame 前，不得修改 UI 代码、安装运行时依赖或开始视觉重构。遇到 Auth required 时停止并提示我刷新 token；不得索取、输出或记录 token。
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
- 旧归档中的虚构/过时功能已被识别并记录到产品需求的未来候选章节。
- 三份新文档已经用户确认；旧归档及相关引用已一次性清理。
- 在 Figma frame 获得明确批准前，不开始代码重构。

## 9. Stitch 执行记录

本节是 Stitch 自动化运行状态的唯一记录，不另建计划文档。每次只在实际 MCP 调用成功后填写 ID；不得预填或猜测。

| 日期 | Project | Design System | Screen | Device / Theme / Model | 结论 | 下一步 |
| --- | --- | --- | --- | --- | --- | --- |
| - | 待创建 | 待创建 | 待生成 | `DESKTOP` / dark / 待调用时记录 | 待评审 | 执行阶段 C 单页 PoC |
