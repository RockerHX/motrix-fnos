# 交互动画完善执行计划

> 状态：阶段 1、2、3、4 代码与自动化验收已完成；阶段 3/4 人工性能与 WebView 项待验收；阶段 4 已停止  
> 初稿日期：2026-07-14  
> 细化日期：2026-07-15  
> 适用范围：阶段 13 UI 重设计中的交互反馈、内容切换、实时进度与弹窗过渡  
> 实施前提：继续受 `docs/development-plan.md` 的 Stitch/Figma 批准门禁约束

## 1. 结论与实施目标

本轮应做一次小范围、系统化的交互动效治理，但不做装饰性动画，也不借机重构页面或业务状态。

完成后应达到以下结果：

1. 自定义控件使用同一套 motion token，不再出现未定义 token 或常规时长散落在组件中的情况。
2. 导航、顶栏、任务操作和移动端浮动创建按钮具备稳定的 focus-visible、pressed、disabled 反馈。
3. 用户切换任务分类时，顶部标题和主内容以同一节奏完成一次短过渡。
4. SSE 只更新速度、进度、ETA、错误文案等字段时，不重放任务项或整页入场动画。
5. 进度条改为 `transform: scaleX(...)` 驱动，不再动画 Naive UI 内部的 `max-width`，同时保留归一化、不倒退、完成态和未知大小语义。
6. `prefers-reduced-motion: reduce` 下，自定义过渡和 Naive UI 的非必要 transition 立即完成；按钮 loading 等必要状态指示仍然可见。
7. 弹窗只做一致性核对和必要修正，不统一重写，不依赖 Naive UI 私有 DOM。

本计划只涉及 Vue、CSS 和前端测试，不修改 Pinia 业务模型、HTTP/SSE、后端、SQLite、打包或 fnOS 运行逻辑。

## 2. 唯一执行顺序

实现时只有以下一条执行主线，不再使用 A/B/C/D 字母编号：

```text
阶段 1：基础规则与高频控件反馈
  ↓ 当前阶段的代码、定向测试、typecheck、verify:pre-commit 全部通过
阶段 2：分类标题、主内容与浮动按钮切换
  ↓ 当前阶段的代码、定向测试、typecheck、verify:pre-commit 全部通过
阶段 3：进度条 transform 化
  ↓ 当前阶段的代码、定向测试、typecheck、verify:pre-commit 全部通过
阶段 4：弹窗与消息反馈核对
  ↓ 最终 pnpm run verify 与 fnOS WebView 人工回归
完成
```

| 执行阶段 | 本阶段目标 | 主要文件 | 进入条件 | 退出条件 |
| --- | --- | --- | --- | --- |
| 阶段 1 | 先建立 motion token、reduced-motion 基线，再统一高频按钮反馈 | `tokens.css`、`base.css`、`SidebarNav.vue`、`Topbar.vue`、`TaskActions.vue`、`MainWindow.vue` | 第 16 节实施门禁已满足；已识别工作区原有改动 | 完成第 7 节全部修改；阶段 1 定向测试、typecheck、`verify:pre-commit` 通过；按钮交互无回归 |
| 阶段 2 | 在阶段 1 的 token 基础上实现分类标题、主内容和 FAB 显隐过渡 | `MainWindow.vue`、`Topbar.vue`、`useTaskCategoryView.spec.ts` 及对应测试 | 阶段 1 退出条件全部满足 | 完成第 8 节全部修改；阶段 2 自动化和人工检查通过；SSE 字段更新不触发整表过渡 |
| 阶段 3 | 将任务进度条替换为 transform 驱动，同时保留进度业务语义 | `TaskProgressBar.vue`、`TaskProgressCell.vue` 及对应测试 | 阶段 2 退出条件全部满足 | 完成第 9 节全部修改；阶段 3 自动化和性能检查通过；进度不倒退且完成态正确 |
| 阶段 4 | 核对弹窗、消息、焦点和关闭保护，仅修复实际发现的问题 | 弹窗封装、直接使用 `NModal` 的组件及对应测试 | 阶段 3 退出条件全部满足 | 完成第 10 节核对；必要修正和测试通过；最终 `pnpm run verify` 通过；剩余 fnOS 实机项已明确记录 |

强制执行规则：

1. 必须从阶段 1 开始，按 `1 → 2 → 3 → 4` 顺序执行，禁止跳阶段或并行实施多个阶段。
2. 阶段内部也必须按小节顺序执行，例如阶段 1 必须按 `7.1 → 7.2 → … → 7.7` 完成。
3. 每次只修改当前阶段文件；同一文件跨阶段时，只完成当前阶段明确列出的部分。
4. 当前阶段的测试、检查或回滚条件未解决前，不得开始下一阶段，也不得用后续阶段的修改掩盖问题。
5. 每完成一个阶段，先报告修改文件、自动化结果、人工验收结果和偏离项，等待该阶段确认后再继续。
6. 阶段 4 如果核对后无需生产代码修改，可以不产生代码提交，但必须提交核对结果和测试证据。

## 3. 已核对的当前实现基线

以下结论来自 2026-07-15 的当前工作区，实施前需再次确认文件未发生结构性变化。

| 区域 | 当前实现 | 已确认问题或可复用点 |
| --- | --- | --- |
| motion token | `src/styles/tokens.css` 尚无 motion token | `TaskDesktopCard.vue` 已引用未定义的 `--app-transition-fast` |
| reduced motion | `src/styles/base.css` 尚无 `prefers-reduced-motion` 规则 | 自定义过渡和 Naive UI 过渡都未统一降级 |
| 分类视图 | `useTaskCategoryView.ts` 已提供 `contentViewKey`，格式为 `分类-结构状态` | 可直接作为内容 Transition key，无需新增 Pinia 或持久状态 |
| 主内容 | `MainWindow.vue` 通过 `v-if / v-else` 切换扩展页、空状态、任务表 | 目前没有 Vue Transition；现有 key 已挂在三个内容根组件上 |
| 顶部标题 | `Topbar.vue` 直接渲染 `activeCategoryLabel` | 分类切换时文字瞬时替换 |
| 浮动创建按钮 | `MainWindow.vue` 使用 `v-if` 控制 `.floating-add` | 显示和隐藏瞬时发生；现有可见性逻辑可继续复用 |
| 导航与工具栏 | `SidebarNav.vue`、`Topbar.vue` 使用原生 button | 已有 hover/focus 色彩状态，但无 pressed 位移和 motion token |
| 任务操作 | `TaskActions.vue` 使用 Naive UI `NButton` | loading/disabled 语义已存在；只需补统一 pressed 契约，不覆盖 Naive UI 私有样式 |
| 任务操作显隐 | `TaskDesktopCard.vue` 已写 `opacity` transition | token 未定义导致规则不完整；建立 token 后即可生效 |
| 任务进度 | `TaskProgressBar.vue` 使用 `NProgress`，深层选择器动画内部 `max-width`，并常驻 `will-change` | 违反只动画 transform/opacity 的设计约束；需要替换实现，不应继续加深内部选择器 |
| 进度状态 | `TaskProgressCell.vue` 已实现归一化、不倒退、任务或总大小变化时重置、完成态拉满 | 必须保留这些行为并补足现有测试 |
| 弹窗 | 项目同时存在 `AppDialog` / `AppConfirmDialog` 和直接使用 `NModal` 的组件 | 只按清单核对；不得顺手把所有弹窗迁移到统一组件 |
| Naive UI | 当前版本 `2.44.1`；默认 modal scale/fade transition 为 `0.2s` | 已满足弹窗不超过 `200ms`；当前没有必要修改 `NaiveProvider.vue` |

现有测试基座包括：

- `src/views/MainWindow.spec.ts`
- `src/layouts/SidebarNav.spec.ts`
- `src/layouts/Topbar.spec.ts`
- `src/features/tasks/composables/useTaskCategoryView.spec.ts`
- `src/features/tasks/components/TaskActions.spec.ts`
- `src/features/tasks/components/TaskProgressBar.spec.ts`
- `src/features/tasks/components/TaskProgressCell.spec.ts`
- `src/components/ui/AppDialog.spec.ts`
- `src/components/ui/AppConfirmDialog.spec.ts`

实施时应扩展这些测试，不另建重复测试框架。

## 4. 硬性设计与工程约束

### 4.1 动效约束

1. 只允许补间 `transform` 和 `opacity`。
2. hover 的颜色、背景、边框和 outline 可以变化，但必须即时切换，不为这些属性添加 transition。
3. pressed 只允许 `translateY(1px)`，不得使用 `scale()`，不得移动任务卡片。
4. 常规反馈使用 `120ms`，内容切换使用 `160ms`，弹窗上限 `200ms`，进度补间使用 `360ms`。
5. easing 统一为 `cubic-bezier(0, 0, 0.2, 1)`，与当前 Naive UI ease-out 一致。
6. 不使用 `transition: all`、弹簧、stagger、视差、滚动动画、数字滚动、列表高度收缩或持续循环的装饰动画。
7. Vue Transition 不设置 `appear`，避免应用首次加载时播放整页入场。
8. 离场元素在过渡期间必须 `pointer-events: none`，动画不能延迟 disabled、loading、请求发送或错误显示。

### 4.2 架构与范围约束

1. `MainWindow.vue` 只做视图编排；不得在其中新增轮询、业务请求或复杂动画状态机。
2. 不新增 Pinia UI 状态，不持久化 motion 偏好；系统偏好直接由 CSS 媒体查询处理。
3. `contentViewKey` 只由分类和 `list / empty / extensions` 结构状态组成，任务速度、百分比、ETA、错误、状态标签不得进入 key。
4. 不使用 `TransitionGroup` 包裹任务列表。
5. 不引入 motion、GSAP 或其他依赖。
6. 不复制、修改或依赖 Naive UI 私有类名；进度条是本轮唯一允许替换 Naive UI 原语的地方，因为公共 API 无法把内部 `max-width` 过渡改为 transform。
7. 不改变布局、颜色体系、文案、触控尺寸和业务流程；focus-visible 的可见性修正属于交互反馈范围。
8. 只修改本计划列出的文件。发现相邻问题时记录，不在本轮顺手处理。

## 5. Motion token 契约

### 5.1 `src/styles/tokens.css`

在现有尺寸和阴影 token 之后新增以下变量，名称和值按此执行：

| Token | 值 | 用途 |
| --- | --- | --- |
| `--app-motion-duration-fast` | `120ms` | pressed、disabled、操作区显隐、浮动按钮 |
| `--app-motion-duration-standard` | `160ms` | 分类标题和主内容切换 |
| `--app-motion-duration-dialog` | `200ms` | 自定义弹窗上限记录；本阶段不强制接管 Naive UI 默认 transition |
| `--app-motion-duration-progress` | `360ms` | SSE 进度值补间 |
| `--app-motion-ease-out` | `cubic-bezier(0, 0, 0.2, 1)` | 所有自定义过渡 |
| `--app-transition-fast` | `var(--app-motion-duration-fast) var(--app-motion-ease-out)` | 兼容现有 `transition: opacity var(--app-transition-fast)` 写法 |
| `--app-transition-standard` | `var(--app-motion-duration-standard) var(--app-motion-ease-out)` | Vue 内容和标题 Transition |
| `--app-transition-progress` | `var(--app-motion-duration-progress) var(--app-motion-ease-out)` | 进度 fill transform |

禁止再添加 `150ms`、`180ms`、`ease-out` 等等价硬编码。Naive UI 自带的内部时长不属于自定义组件硬编码，不用强行覆盖。

### 5.2 `src/styles/base.css`

新增全局 reduced-motion 基线：

```css
@media (prefers-reduced-motion: reduce) {
  :root {
    --app-motion-duration-fast: 0.01ms;
    --app-motion-duration-standard: 0.01ms;
    --app-motion-duration-dialog: 0.01ms;
    --app-motion-duration-progress: 0.01ms;
  }

  *,
  *::before,
  *::after {
    scroll-behavior: auto !important;
    transition-duration: 0.01ms !important;
    transition-delay: 0ms !important;
  }
}
```

执行说明：

- 只覆盖 transition 的时长和延迟，不写 `transition: none` 或 `transition: all`，保证 Vue/Naive UI 的进入离开钩子仍能完成。
- 不全局覆盖 `animation-duration`，避免冻结 Naive UI button loading 等必要的进行中指示。
- 本轮禁止新增非必要 keyframe animation；若后续确有新增，必须在组件本地补 reduced-motion 分支。

## 6. 交互状态行为矩阵

| 场景 | 默认行为 | reduced-motion | 禁止行为 |
| --- | --- | --- | --- |
| hover | 背景/颜色即时变化；可将图标或控件 opacity 在 `120ms` 内补到 `1` | 即时变化 | scale、尺寸变化、阴影大幅漂移 |
| focus-visible | 立即显示清晰 outline；不依赖 hover | 同默认 | `outline: none` 后无替代焦点样式 |
| pressed | `transform: translateY(1px)`，`120ms` | 即时到达/恢复 | scale、任务卡片移动、持续按压动画 |
| disabled | 业务状态立即禁用；opacity 在 `120ms` 内变化 | 即时变化 | disabled 后仍响应 click、位移残留 |
| loading | 继续使用 Naive UI loading；按钮尺寸不变 | loading 指示保留 | 新增脉冲、骨架闪烁或全页遮罩 |
| 分类标题 | opacity + 最多 `2px` Y 位移，`160ms` | 即时替换 | 文字逐字、滑动超过 `4px` |
| 主内容 | opacity + 最多 `4px` Y 位移，`160ms` | 即时替换 | TransitionGroup、列表逐项进入、首屏 appear |
| 浮动创建按钮 | opacity + 最多 `4px` Y 位移，`120ms` | 即时显示/隐藏 | scale、弹簧、持续悬浮 |
| 任务进度 | fill 使用 `scaleX`，`360ms` | 即时更新 | width/max-width 动画、常驻 will-change |
| modal | 保留 Naive UI 默认 opacity/transform，最长 `200ms` | transition 近似即时 | 私有类覆盖、内容 stagger |

## 7. 阶段 1：基础规则与高频控件反馈

阶段 1 执行记录（本节记录暂不提交）：

- [x] 小任务 1.1：建立动效基础规则；commit `a1738ca`；`TaskDesktopCard.spec.ts` 3 项通过，typecheck 通过，提交钩子快速验证通过；完成于 2026-07-15 23:50 CST。
- [x] 小任务 1.2：完善侧栏按钮反馈；commit `cf9544c`；`SidebarNav.spec.ts` 4 项通过，typecheck 通过，提交钩子快速验证通过；完成于 2026-07-15 23:52 CST。
- [x] 小任务 1.3：完善顶栏按钮反馈；commit `4f500e4`；`Topbar.spec.ts` 6 项通过，typecheck 通过，提交钩子快速验证通过；完成于 2026-07-15 23:54 CST。
- [x] 小任务 1.4：统一任务操作按钮反馈；commit `abe0859`；`TaskActions.spec.ts` 10 项通过，typecheck 通过，提交钩子快速验证通过；完成于 2026-07-15 23:57 CST。
- [x] 小任务 1.5：完善移动端浮动创建按钮反馈；commit `ecc0d28`；`MainWindow.spec.ts` 3 项通过，typecheck 通过，提交钩子快速验证通过；首次提交钩子的 Rust 双监听器时序测试瞬时失败，单独复跑与完整重跑均通过；完成于 2026-07-16 00:13 CST。
- [x] 小任务 1.6：阶段 1 总体验收；5 个定向测试文件共 26 项通过，typecheck 通过，`verify:pre-commit` 通过（Rust 192 项、前端 249 项）；静态检查未发现禁用动效规则，暂存区为空；浏览器键盘焦点、reduced-motion 和 fnOS WebView 视觉检查保留给人工验收；完成于 2026-07-16 00:15 CST。

阶段 1 结论：代码实现与自动化验收完成，共产生 5 个可独立回滚的规范提交；本执行记录未提交，阶段 2 尚未开始。

### 7.1 建立 token 与 reduced-motion 基线

修改：

- `src/styles/tokens.css`
- `src/styles/base.css`

执行步骤：

1. 按第 5 节增加 token 和媒体查询。
2. 全仓搜索 `--app-transition-fast`，确认已有引用能够解析。
3. 全仓搜索 `transition:`、`transition-duration`、`animation:` 和 `will-change`，记录本轮范围外的既有规则，但只处理本计划列出的组件。
4. 搜索新增代码，确保没有 `transition: all`、`scale(`、动画 width/height/top/left 或新增 keyframe。

完成标准：

- 所有本轮新增 transition 都引用 token。
- `TaskDesktopCard.vue` 的现有 opacity 过渡因 token 定义而生效。
- reduced-motion 开启后，自定义过渡和 Naive UI modal/message 的 transition 近似即时，button loading 仍可辨识。

### 7.2 `src/layouts/SidebarNav.vue`

修改原生导航和 footer button 的交互规则：

1. 为 `.category-list button, .sidebar-footer button` 增加 `opacity` 和 `transform` transition，只使用 `--app-transition-fast`。
2. 增加 `:active:not(:disabled) { transform: translateY(1px); }`。
3. disabled 必须恢复 `transform: none`，避免按下时状态切换导致位移残留。
4. 保留 active 导航条和现有 hover 背景，不给 background/color 添加 transition。
5. 将当前 hover 与 focus-visible 共用规则拆开或补充独立 focus-visible outline，保证键盘焦点不只靠轻微背景差异表达。
6. 手机底部导航沿用同一 pressed 规则，不增加移动端专用动画。

测试：

- 扩展 `SidebarNav.spec.ts`：补充分类按钮 click 只发出一次 `selectCategory`，当前项仍保持 `aria-current="page"`，logout loading 时按钮 disabled 且不发出 logout。
- 不在 jsdom 中断言 CSS 时长或 transform；这些由静态检查和人工验收覆盖。

### 7.3 `src/layouts/Topbar.vue`

修改顶栏原生按钮：

1. 为 `.topbar-actions > button` 增加 opacity/transform transition。
2. 增加统一 pressed 位移；disabled 时强制 `transform: none`。
3. 保留主创建按钮和普通按钮现有颜色、背景、尺寸与阴影，不动画这些属性。
4. 为 focus-visible 提供明确 outline，不以 hover 背景代替键盘焦点。
5. 桌面与移动按钮共用同一规则，不能只修桌面工具栏。

测试：

- 保留现有操作顺序、事件、英文 title/aria-label、Trash 分支和 disabled 用例。
- 新增 logout loading 用例，证明移动端退出按钮 disabled 后不会发出 logout。
- 标题 Transition 的测试放在阶段 2，不在本步骤提前改模板。

### 7.4 `src/features/tasks/components/TaskActions.vue`

修改 Naive UI 任务操作按钮：

1. 给三种分支中的所有操作 `NButton` 增加统一类名，例如 `task-action-button`；不能只覆盖 `icon-pill` 分支。
2. 只在该公共类上增加 `transform` transition 和 pressed 位移。
3. 使用 `:not(:disabled)` 或 Naive UI 根按钮的公开 disabled 状态，确保 loading/disabled 时不位移。
4. 不通过 `:deep()` 覆盖 Naive UI ripple、spinner、内部 label 或私有类。
5. 不改变按钮权限、弹窗开关、loading 和 emit 行为。

测试：

- 运行并保留 `TaskActions.spec.ts` 的三种布局、权限、loading、runtime exiting 和确认弹窗用例。
- 只在必要时补充所有分支都带公共类的轻量断言；不要断言 Naive UI 内部 DOM。

### 7.5 `src/views/MainWindow.vue` 的浮动创建按钮

本阶段只处理按钮本身的交互状态；显示/隐藏 Transition 在阶段 2 完成。

1. 为 `.floating-add` 增加 opacity/transform transition 和 pressed 位移。
2. 增加可见 focus-visible outline。
3. 保持现有绝对定位、移动端安全区、尺寸、颜色和 click 行为。
4. 不修改 `showFloatingAdd` 的业务条件。

### 7.6 本阶段只验证、不修改的文件

- `src/features/tasks/components/TaskDesktopCard.vue`：现有 `transition: opacity var(--app-transition-fast)` 在 token 建立后应直接恢复；除非验证失败，否则不改文件。
- `src/app/providers/NaiveProvider.vue`：当前 Naive UI ease-out 和 modal `0.2s` 已符合要求；不要添加无效或不存在的 duration theme override。

### 7.7 阶段 1 验证命令

```bash
pnpm exec vitest run \
  src/layouts/SidebarNav.spec.ts \
  src/layouts/Topbar.spec.ts \
  src/features/tasks/components/TaskActions.spec.ts \
  src/views/MainWindow.spec.ts
pnpm run typecheck
pnpm run verify:pre-commit
```

阶段 1 回滚条件：出现按钮点击事件丢失、loading spinner 不可见、disabled 仍触发事件、手机底栏尺寸变化或 focus outline 被裁切时，修复或回滚阶段 1，不得进入阶段 2。

## 8. 阶段 2：分类标题、主内容与浮动按钮切换

阶段 2 执行记录（本节记录暂不提交）：

- [x] 小任务 2.1：固定内容切换 key 契约；commit `45690cd`；`useTaskCategoryView.spec.ts` 5 项通过，typecheck 通过，提交钩子快速验证通过；完成于 2026-07-16 00:25 CST。
- [x] 小任务 2.2：添加主内容分类切换过渡；commit `8052f47`；`MainWindow.spec.ts` 与 `useTaskCategoryView.spec.ts` 共 10 项通过，typecheck 通过，提交钩子快速验证通过；完成于 2026-07-16 00:29 CST。
- [x] 小任务 2.3：添加顶栏分类标题过渡；commit `00b5382`；`Topbar.spec.ts` 8 项通过，typecheck 通过，提交钩子快速验证通过；完成于 2026-07-16 00:31 CST。
- [x] 小任务 2.4：添加浮动按钮显隐过渡；commit `de24882`；`MainWindow.spec.ts` 与 `useTaskCategoryView.spec.ts` 共 11 项通过，typecheck 通过，提交钩子快速验证通过；完成于 2026-07-16 00:33 CST。
- [x] 小任务 2.5：阶段 2 总体验收；3 个定向测试文件共 19 项通过，typecheck 通过，`verify:pre-commit` 通过（Rust 192 项、前端 256 项）；静态检查未发现禁用动效规则，暂存区为空；快速分类切换、30 秒 SSE、reduced-motion 和 fnOS WebView 视觉检查保留给人工验收；完成于 2026-07-16 00:35 CST。

阶段 2 结论：代码实现与自动化验收完成，共产生 4 个可独立回滚的规范提交；本执行记录未提交，阶段 3 尚未开始。

### 8.1 主内容 Transition：`src/views/MainWindow.vue`

使用 Vue 内置 `<Transition name="app-content-switch">` 包裹现有三个互斥分支：

- `ExtensionsPlaceholder`
- `TaskEmptyState`
- `TaskTable`

实现契约：

1. 三个分支继续直接使用现有 `:key="contentViewKey"`。
2. 不增加 `appear`，不使用 `TransitionGroup`。
3. 不增加业务 wrapper；若为 transition 布局必须增加 wrapper，先证明不会改变 `TaskTable` 的高度、滚动容器和分页布局，否则使用离场元素 absolute 的方式解决重叠。
4. enter-from 为 `opacity: 0` 和最多 `translateY(4px)`；leave-to 为 `opacity: 0` 和最多反向 `2px`。
5. enter/leave 同时进行，总时长为 `160ms`，不使用 `mode="out-in"` 造成两段累计等待。
6. leave-active 元素设为 absolute、占满当前 content stage，并禁用 pointer events；新内容保持正常文档流，避免切换期间高度抖动。
7. Transition class 放在 `MainWindow.vue` scoped style 或 `base.css` 均可，但同一组 class 只定义一次。

`contentViewKey` 的允许变化规则：

| 变化 | key 是否变化 | 是否播放内容过渡 |
| --- | --- | --- |
| 用户切换 `all -> downloading` | 是 | 是，一次 |
| 用户切换 `trash -> extensions` | 是 | 是，一次 |
| 当前分类 `empty -> list` | 是 | 是，一次 |
| 当前分类最后一个匹配任务离开，`list -> empty` | 是 | 是，一次；这是结构变化 |
| completedLength、downloadSpeed、ETA、errorMessage 更新 | 否 | 否 |
| 列表内任务数量变化但仍为 list | 否 | 否 |
| 分页 page/pageSize 更新 | 否 | 否 |

禁止为了区分“用户更新”和“SSE 更新”新增计时器、随机 key、时间戳 key 或 Pinia motion flag。现有结构 key 已足够。

### 8.2 顶部标题 Transition：`src/layouts/Topbar.vue`

1. 在标题文字外增加固定布局容器，例如 `.topbar-title-label`，容器保持当前标题行高和宽度行为。
2. 使用 `<Transition name="app-title-switch">` 包裹 `<strong :key="props.activeCategory">`。
3. enter/leave 使用与主内容相同的 `160ms` token，位移不超过 `2px`。
4. 离场标题 absolute，新标题保持正常流，避免两份标题同时撑宽顶栏。
5. `activeCategoryLabel` 的翻译与 aria 行为保持不变。
6. 不为语言切换另建动画状态；若语言变化而 category key 不变，标题直接更新即可。

### 8.3 浮动按钮显示/隐藏：`src/views/MainWindow.vue`

1. 用 `<Transition name="app-floating-add">` 包裹现有 `.floating-add` button。
2. 保留原 `v-if="isMobileLayout && showFloatingAdd"`，不更改判断条件。
3. enter-from/leave-to 使用 `opacity: 0` 和最多 `translateY(4px)`，时长 `120ms`。
4. leave-active 期间 `pointer-events: none`。
5. 不使用 scale，不能改变按钮最终位置或安全区计算。

### 8.4 测试增量

`src/features/tasks/composables/useTaskCategoryView.spec.ts`：

1. 保留现有分类筛选、empty/list key 和 FAB 可见性测试。
2. 新增 key 稳定性测试：修改同一任务的 `completedLength`、`downloadSpeed`、`errorMessage`，只要结构仍是 list，`contentViewKey` 保持不变。
3. 新增结构变化测试：空数组变为有任务、最后一个匹配任务离开当前分类时，key 只在 `empty/list` 间变化一次。
4. 不测试 CSS class 的毫秒数。

`src/views/MainWindow.spec.ts`：

1. 扩展 `AppShell` stub，使其能发出 `select-category`。
2. 验证分类切换后渲染正确的 empty/list/extensions 分支，现有浮动创建按钮可见性不回归。
3. 使用稳定的 `data-test` 或组件存在性断言，不断言 Vue Transition 的内部注释节点。
4. 普通任务字段更新后，任务表组件仍是同一结构分支；不要使用真实计时等待 CSS transition。

`src/layouts/Topbar.spec.ts`：

1. `setProps({ activeCategory: ... })` 后标题和 Trash 操作分支正确更新。
2. 中英文标题仍完整，不因新增 title wrapper 产生重复文本。

### 8.5 阶段 2 验证命令

```bash
pnpm exec vitest run \
  src/features/tasks/composables/useTaskCategoryView.spec.ts \
  src/views/MainWindow.spec.ts \
  src/layouts/Topbar.spec.ts
pnpm run typecheck
pnpm run verify:pre-commit
```

人工检查至少覆盖：

- `all / downloading / completed / trash / extensions` 连续切换。
- empty、list 和 extensions 三种结构互切。
- 分类切换过程中快速再次点击另一分类。
- SSE 连续更新任务速度和进度至少 30 秒，确认任务表不反复淡入。
- 移动端 FAB 在空状态、任务列表、Trash、Extensions、Runtime exiting 间显示/隐藏正确。

阶段 2 回滚条件：内容切换出现双滚动条、旧内容可点击、分页跳位、任务表高度塌陷、快速切换后显示错误分类或 SSE 字段更新触发整表淡入时，修复或回滚阶段 2，不得进入阶段 3。

## 9. 阶段 3：进度条 transform 化

### 9.1 技术决策

不继续使用 `NProgress` 的当前 line fill 过渡，原因是 Naive UI `2.44.1` 的公开 API 不能把内部 `max-width` 补间替换为 transform；继续使用深层私有选择器会扩大升级风险。

在 `TaskProgressBar.vue` 内实现最小、语义化的自定义进度轨道：

```text
.task-progress-bar[role="progressbar"]
  └─ .task-progress-bar__fill
```

轨道负责高度、圆角、裁剪和 rail 背景；fill 始终为 `width: 100%`，通过 `transform: scaleX(var(--task-progress-scale))` 表示进度，`transform-origin: left center`。

### 9.2 `src/features/tasks/components/TaskProgressBar.vue`

具体修改：

1. 移除 `NProgress` import、组件使用和 `:deep(.n-progress-graph-line-fill)`。
2. 移除 `transitionMs` prop，避免组件 API 与全局 progress token 并存两套时长来源。
3. 保留 percentage 的 finite 检查与 `0..100` clamp。
4. 计算 `scale = normalizedPercentage / 100`；empty tone 固定为 `0`。
5. 根元素保留现有 variant/tone class，并增加：
   - `role="progressbar"`
   - `aria-valuemin="0"`
   - `aria-valuemax="100"`
   - `aria-valuenow`：已知大小时使用归一化百分比，未知大小时使用 `0`，与现有 `NProgress` 的公开语义保持一致
6. 使用 CSS 自定义属性传递 scale，不把 transition 字符串写入 inline style。
7. compact 高度继续为 `5px`，card 高度继续为 `4px`。
8. default 与 complete tone 保留当前绿色渐变差异；empty tone 保留当前条纹 rail，不增加流动条纹。
9. fill 只 transition transform，使用 `--app-transition-progress`。
10. 删除 `will-change`；不得在长任务列表中为每个进度条常驻合成层。
11. 不动画百分比文字，不改变 `TaskProgressCell` 的 `toFixed(2)` 展示。

实现后应满足的 CSS 结构：

```css
.task-progress-bar__fill {
  width: 100%;
  height: 100%;
  transform: scaleX(var(--task-progress-scale));
  transform-origin: left center;
  transition: transform var(--app-transition-progress);
}
```

如果 scaleX 导致低百分比下圆角或渐变明显失真，先停在该单组件验证，不改用 width/max-width/clip-path 动画。可通过调整静态背景和 overflow 结构修正，但仍只能补间 transform。

### 9.3 `src/features/tasks/components/TaskProgressCell.vue`

1. 删除 `TRANSITION_MS` 常量和传给 `TaskProgressBar` 的 `transition-ms`。
2. 保留现有 watch 的五项输入：task id、gid、status、totalLength、completedLength。
3. 保留以下状态规则：
   - 非有限值或越界值归一化。
   - 同一任务、同一 gid、同一 totalLength 下，旧 SSE 事件不得让显示进度倒退。
   - task id 或 gid 改变时允许重置。
   - totalLength 改变时按新总量重置。
   - totalLength 小于等于 0 时显示 empty tone 和 0%。
   - status 变为 complete 且 totalLength 大于 0 时强制 100%。
4. 不新增 requestAnimationFrame、setInterval、JS tween 或基于 SSE 间隔的计时器。

### 9.4 测试增量

重写 `TaskProgressBar.spec.ts` 中依赖 `NProgress` stub 的断言，改为测试公开 DOM 契约：

1. `P-BAR-01`：负数、超过 100、NaN/Infinity 都归一化到合法 scale 和 aria value。
2. `P-BAR-02`：50% 对应 `--task-progress-scale: 0.5`。
3. `P-BAR-03`：empty tone 的 scale 和 aria value 为 0，轨道 class 正确。
4. `P-BAR-04`：default/complete tone class 和 fill 存在，视觉渐变由 class/CSS 管理，不断言整段内部样式字符串。
5. `P-BAR-05`：compact/card variant class 正确；高度属于 CSS 人工验收，不依赖 jsdom computed style。
6. `P-BAR-06`：组件不再依赖或渲染 `NProgress`。

扩展 `TaskProgressCell.spec.ts`：

1. `P-CELL-01`：同一任务进度从 20% 增到 40%。
2. `P-CELL-02`：随后收到 30% 的旧事件，显示仍为 40%。
3. `P-CELL-03`：task id 或 gid 改变后允许从较低值重新开始。
4. `P-CELL-04`：totalLength 改变后按新总量重算。
5. `P-CELL-05`：unknown total 为 empty/0，变为已知 total 后恢复正常。
6. `P-CELL-06`：complete 强制 100%。
7. `P-CELL-07`：showLabel false 时不渲染百分比文本。

### 9.5 阶段 3 验证命令

```bash
pnpm exec vitest run \
  src/features/tasks/components/TaskProgressBar.spec.ts \
  src/features/tasks/components/TaskProgressCell.spec.ts \
  src/features/tasks/components/TaskDesktopCard.spec.ts \
  src/features/tasks/components/TaskMobileList.spec.ts
pnpm run typecheck
pnpm run verify:pre-commit
```

人工性能检查：

1. 使用长列表同时观察多个 active task，连续更新至少 60 秒。
2. 快速进度增长、低速增长、接近 100%、完成态、未知大小切换都不得闪烁或倒退。
3. 滚动时不得出现明显掉帧；DevTools Layers 不应为每个空闲进度条保留由 `will-change` 创建的常驻层。
4. 页面空闲时不得存在进度相关 interval、requestAnimationFrame 或持续 animation。
5. reduced-motion 开启后进度立即跳到新值，关闭后恢复 `360ms` 补间。

阶段 3 回滚条件：进度语义丢失、旧事件导致倒退、完成态不是 100%、低百分比视觉严重失真、长列表滚动明显变差或必须依赖 Naive UI 私有选择器才能完成时，回滚本阶段并记录技术验证结果；不得进入阶段 4。

### 9.6 阶段 3 执行记录（本节记录暂不提交）

- [x] 小任务 3.1：建立并实现进度条公开 DOM 契约；commit `6b7d667`；`TaskProgressBar.spec.ts` 6 项通过，`pnpm run typecheck` 通过，提交钩子快速验证通过（Rust 192 项、前端 258 项）；完成于 2026-07-16 08:43 CST。
- [x] 小任务 3.2：清理 TaskProgressCell 局部时长并固化状态语义；commit `ab2769a`；`TaskProgressCell.spec.ts` 11 项通过，`pnpm run typecheck` 通过，提交钩子快速验证通过（Rust 192 项、前端 266 项）；完成于 2026-07-16 08:46 CST。
- [x] 小任务 3.3：固化桌面与移动任务列表的进度组件契约；commit `63aac90`；桌面/移动消费者 spec 共 5 项通过，`pnpm run typecheck` 通过，提交钩子快速验证通过（Rust 192 项、前端 266 项）；完成于 2026-07-16 08:48 CST。
- [x] 小任务 3.4：阶段 3 总体验收；四个定向测试文件共 22 项通过，`pnpm run typecheck` 通过，`pnpm run verify:pre-commit` 通过（构建脚本 18 项、Rust 192 项、前端 266 项）；静态扫描确认仅保留 `transform: scaleX(...)` 进度过渡，暂存区为空；完成于 2026-07-16 08:51 CST。

阶段 3 结论：三个代码提交可独立回滚，进度归一化、不倒退、任务/总量重置、unknown total 和完成态语义均有自动化覆盖；长列表 60 秒性能、reduced-motion 实机视觉和 fnOS WebView 视觉表现仍需用户人工验收。阶段 4 已完成自动化核对，本执行记录继续保持未提交。

## 10. 阶段 4：弹窗与消息反馈核对

### 10.1 默认策略

当前 Naive UI `2.44.1` modal 默认使用约 `0.2s` 的 opacity/transform transition，已经满足本项目上限。`NaiveProvider.vue` 的 common ease-out 也与计划 token 一致。

因此默认策略是：

1. 保留 Naive UI transition。
2. 依赖第 5.2 节的 reduced-motion 全局 transition-duration 降级。
3. 不新增 modal 私有类覆盖，不统一迁移全部直接 `NModal`。
4. 只有发现具体的遮罩脱节、关闭保护、焦点恢复或时长回归时，才修改对应组件。

### 10.2 核对清单与预期处置

| 文件 | 当前形态 | 核对重点 | 默认处置 |
| --- | --- | --- | --- |
| `src/components/ui/AppDialog.vue` | `NModal + NCard` 通用封装 | mask close、closeDisabled、header close、focus restore | 保留；只补回归测试或实际缺陷 |
| `src/components/ui/AppConfirmDialog.vue` | 基于 AppDialog | loading/disabled 期间 cancel、mask、close 均不可用 | 保留现有职责 |
| `src/features/tasks/components/TaskCreateDialog.vue` | 直接 NModal | creating/runtime exiting 时 mask 和关闭按钮保护 | 保留；验证 `isMaskClosable` 与 closeDialog 一致 |
| `src/features/tasks/components/TaskDetailsDialog.vue` | 直接 NModal | 打开/关闭、Esc/mask、焦点返回触发按钮 | 无 loading，不为统一而迁移 |
| `src/features/diagnostics/components/DebugLogDialog.vue` | 直接 NModal | refresh/clear 操作中关闭是否安全、遮罩与卡片同步 | 先验证；只有会产生状态错误时才阻止关闭 |
| `src/features/diagnostics/components/DebugLogManualCopyDialog.vue` | 直接 NModal | after-enter focus、关闭后焦点恢复 | 保留手动复制流程 |
| `src/features/settings/components/JsonRpcTokenSettings.vue` | preset card NModal | isSaving 时 mask/closable/cancel 一致 | 保留现有保护 |
| `src/features/auth/components/WebAuthSettings.vue` | 两个 preset card NModal | isSubmitting 时两个弹窗的 mask/closable/cancel 一致 | 保留现有保护 |

通过 `AppDialog` / `AppConfirmDialog` 间接覆盖的 About、Help、Settings、Diagnostics、批量删除、重新下载、文件确认、永久删除和删除确认弹窗，只做抽样回归，不逐个重写。

### 10.3 焦点和关闭行为验收

每类弹窗至少验证：

1. 由按钮打开后，键盘焦点进入弹窗的合理控件。
2. Tab/Shift+Tab 不会把焦点丢到遮罩后的任务列表。
3. 允许关闭时，Esc、遮罩、header close 和 cancel 的行为一致。
4. loading/disabled 明确禁止关闭的弹窗，所有关闭入口都被阻止；业务状态立即生效，不等动画结束。
5. 关闭动画结束后焦点回到原触发按钮或合理替代点。
6. 快速开关、连续打开不同弹窗、弹窗内再打开手动复制弹窗时，遮罩层级和焦点不丢失。
7. reduced-motion 开启后以上行为不变，只缩短视觉过渡。

不要为了测试焦点而增强现有 NModal stub 去模拟 Naive UI 全部内部行为。单元测试覆盖项目自己的 closeDisabled/emit 契约，真实 focus trap 和 restore 由浏览器人工验收。

### 10.4 Message 核对

1. 继续使用 Naive UI `NMessageProvider`，不自建 Toast 动画队列。
2. reduced-motion 下 message 的进入离开近似即时，但显示时长和业务触发次数不变。
3. SSE 刷新不得重复生成同一任务的动效型 message；若发现重复消息，属于现有 toast 去重逻辑问题，单独记录，不在 motion CSS 中掩盖。

### 10.5 阶段 4 验证

```bash
pnpm exec vitest run \
  src/components/ui/AppDialog.spec.ts \
  src/components/ui/AppConfirmDialog.spec.ts \
  src/features/tasks/components/TaskCreateDialog.spec.ts \
  src/features/tasks/components/TaskActions.spec.ts \
  src/features/diagnostics/components/DebugLogDialog.spec.ts \
  src/features/diagnostics/components/DebugLogManualCopyDialog.spec.ts \
  src/features/settings/components/JsonRpcTokenSettings.spec.ts \
  src/features/auth/components/WebAuthSettings.spec.ts
pnpm run typecheck
pnpm run verify:pre-commit
```

若清单中的某个 spec 文件实际不存在，不为凑命令创建空测试；先确认该组件是否由更高层测试覆盖，再只为需要修改的组件补测试。

阶段 4 回滚条件：焦点无法恢复、loading 中可关闭导致状态异常、遮罩比内容提前消失、嵌套弹窗层级错误或必须依赖 Naive UI 私有类才能统一时，回滚对应组件的修改；其他已通过组件不受影响。

### 10.6 阶段 4 执行记录（本节记录暂不提交）

- [x] 小任务 4.1：固化通用弹窗关闭契约；commit `2e8ff40`；`AppDialog.spec.ts` 与 `AppConfirmDialog.spec.ts` 共 9 项通过，`pnpm run typecheck` 通过，提交钩子验证通过；仅修改测试，完成于 2026-07-16 09:52 CST。
- [x] 小任务 4.2：固化任务创建与详情弹窗关闭行为；commit `1d546ea`；`TaskCreateDialog.spec.ts` 与 `TaskDetailsDialog.spec.ts` 共 8 项通过，`pnpm run typecheck` 通过，提交钩子验证通过（Rust 192 项、前端 272 项）；仅修改测试，完成于 2026-07-16 09:55 CST。
- [x] 小任务 4.3：固化诊断弹窗嵌套、关闭与焦点契约；commit `bc034ee`；`DebugLogDialog.spec.ts` 与 `DebugLogManualCopyDialog.spec.ts` 共 6 项通过，`pnpm run typecheck` 通过，提交钩子验证通过（Rust 192 项、前端 276 项）；仅修改测试，完成于 2026-07-16 09:58 CST。
- [x] 小任务 4.4：固化 Token 与认证弹窗 loading 保护；commit `72e924b`；`JsonRpcTokenSettings.spec.ts` 与 `WebAuthSettings.spec.ts` 共 9 项通过，`pnpm run typecheck` 通过，提交钩子验证通过（Rust 192 项、前端 278 项）；仅修改测试，完成于 2026-07-16 10:08 CST。
- [x] 小任务 4.5：固化消息触发与 reduced-motion 边界；commit `d6d57c8`；四个消息相关 spec 共 10 项通过，`pnpm run typecheck` 通过，提交钩子验证通过（Rust 192 项、前端 280 项）；仅修改测试，完成于 2026-07-16 10:11 CST。
- [x] 小任务 4.6：阶段 4 总体验收；13 个定向 spec 共 52 项通过，`pnpm run typecheck` 通过，`pnpm run verify:pre-commit` 通过（构建脚本 18 项、Rust 192 项、前端 280 项）；静态扫描无新增禁止模式，暂存区为空，完成于 2026-07-16 10:13 CST。

阶段 4 结论：五个核对提交均为测试-only、可独立回滚，未修改生产组件、公共 API、依赖或后端；通用弹窗、任务弹窗、诊断嵌套弹窗、Token/认证 loading 保护和消息触发边界均有自动化证据。真实浏览器与 fnOS WebView 的焦点恢复、遮罩层级、reduced-motion 和视觉时长仍需用户人工验收；阶段 4 自动化完成后停止，不进入后续阶段。本执行记录继续保持未提交。

## 11. 文件级修改矩阵

| 文件 | 阶段 | 计划动作 | 必须修改 |
| --- | --- | --- | --- |
| `src/styles/tokens.css` | 1 | 增加 motion token | 是 |
| `src/styles/base.css` | 1、2 | 阶段 1 增加 reduced-motion；阶段 2 仅在需要时增加可复用 Transition class | 是 |
| `src/layouts/SidebarNav.vue` | 1 | pressed、opacity/transform、focus-visible | 是 |
| `src/layouts/SidebarNav.spec.ts` | 1 | 事件、aria-current、disabled 回归 | 是 |
| `src/layouts/Topbar.vue` | 1、2 | 阶段 1 处理按钮反馈；阶段 2 增加标题 Transition | 是 |
| `src/layouts/Topbar.spec.ts` | 1、2 | 阶段 1 验证 disabled/logout；阶段 2 验证标题切换 | 是 |
| `src/views/MainWindow.vue` | 1、2 | 阶段 1 处理 FAB 按钮反馈；阶段 2 增加 FAB 显隐和内容 Transition | 是 |
| `src/views/MainWindow.spec.ts` | 1、2 | 阶段 1 做按钮回归；阶段 2 验证分类分支与 FAB 显隐 | 是 |
| `src/features/tasks/composables/useTaskCategoryView.ts` | 2 | 复用现有 key；仅发现契约不满足时修改 | 默认否 |
| `src/features/tasks/composables/useTaskCategoryView.spec.ts` | 2 | key 稳定性与结构变化测试 | 是 |
| `src/features/tasks/components/TaskActions.vue` | 1 | 三分支统一 action class 与 pressed | 是 |
| `src/features/tasks/components/TaskActions.spec.ts` | 1、4 | 阶段 1 做行为回归；阶段 4 仅在弹窗核对需要时扩展 | 视修改而定 |
| `src/features/tasks/components/TaskDesktopCard.vue` | 1 | 验证现有 token 引用恢复 | 默认否 |
| `src/features/tasks/components/TaskProgressBar.vue` | 3 | 用 transform 自定义轨道替换 NProgress | 是 |
| `src/features/tasks/components/TaskProgressBar.spec.ts` | 3 | 改为公开 DOM/aria/scale 契约 | 是 |
| `src/features/tasks/components/TaskProgressCell.vue` | 3 | 删除局部时长，保留状态逻辑 | 是 |
| `src/features/tasks/components/TaskProgressCell.spec.ts` | 3 | 补不倒退、重置、完成态测试 | 是 |
| `src/app/providers/NaiveProvider.vue` | 4 | 仅核对，不添加 duration override | 默认否 |
| `src/components/ui/AppDialog.vue` | 4 | 只修实际发现的问题 | 默认否 |
| 直接使用 `NModal` 的 6 类组件 | 4 | 按清单核对，逐个决定 | 默认否 |

若实现模型准备修改矩阵之外的生产文件，必须先说明该文件与验收项的直接关系，并更新本计划；不能先改后补理由。

## 12. 自动化与人工验收矩阵

### 12.1 自动化验收

| 编号 | 验收项 | 证据 |
| --- | --- | --- |
| AUTO-01 | motion token 存在且 `--app-transition-fast` 可解析 | token 文件检查、type/build 通过 |
| AUTO-02 | 分类字段更新不会改变 content key | `useTaskCategoryView.spec.ts` |
| AUTO-03 | empty/list/extensions 结构 key 正确变化 | composable + MainWindow 测试 |
| AUTO-04 | 顶栏分类标题与操作分支正确更新 | `Topbar.spec.ts` |
| AUTO-05 | disabled/loading 不产生额外事件 | Sidebar/Topbar/TaskActions/Dialog 测试 |
| AUTO-06 | 进度 clamp、scale、tone、aria 契约正确 | `TaskProgressBar.spec.ts` |
| AUTO-07 | 进度不倒退并能按 id/gid/total 重置 | `TaskProgressCell.spec.ts` |
| AUTO-08 | 弹窗关闭保护不回归 | AppDialog/AppConfirm 及被修改组件测试 |
| AUTO-09 | 没有新增依赖或后端改动 | git diff、lockfile 检查 |
| AUTO-10 | 前端全量快速验证通过 | `pnpm run verify:pre-commit` |

### 12.2 视觉与交互验收

| 维度 | 必测组合 |
| --- | --- |
| 视口 | `390x844`、`1024x768`、`1440x900` |
| 主题 | 当前已实现主题；阶段 13 浅色完成后补深色/浅色全矩阵 |
| 语言 | 简体中文、英文 |
| 输入 | 鼠标 hover/click、键盘 Tab/Enter/Space/Esc、移动端触控 |
| 状态 | Loading、Empty、Error、Disabled、Selected、Runtime exiting |
| 内容 | all、downloading、completed、trash、extensions；empty/list 切换 |
| 实时更新 | completedLength、downloadSpeed、ETA、status、errorMessage 连续 SSE 更新 |
| 进度 | 0%、低百分比、增长、旧事件倒退、未知大小、完成态 |
| 降级 | 系统 reduced-motion 开/关各走一次完整主流程 |
| 性能 | 长任务列表滚动、60 秒连续进度更新、页面空闲无持续 motion 循环 |

人工验收时只评价是否清楚、稳定、无回归，不追求“动画更明显”。如果需要录屏才能看清楚某个常规反馈，该反馈可能已经过度，应优先减弱或移除。

## 13. 阶段提交边界与最终回归

四个提交边界与四个执行阶段完全一一对应，不构成第二套执行顺序：

1. 阶段 1 — `motion: add tokens and control feedback`
   - token、reduced-motion、Sidebar、Topbar button、TaskActions、FAB pressed。
2. 阶段 2 — `motion: add category and fab transitions`
   - MainWindow 内容、Topbar 标题、FAB 显隐、key 契约测试。
3. 阶段 3 — `motion: make task progress transform-driven`
   - TaskProgressBar、TaskProgressCell 及其测试。
4. 阶段 4 — `motion: align dialog behavior`
   - 只包含弹窗审计后确有必要的修正；若无需修改，可不产生提交。

每个提交必须：

- 只包含当前阶段文件，不混入工作区已有的 package、packaging 或 server 改动。
- 在提交前查看 `git diff --stat` 和逐文件 diff。
- 通过当前阶段定向测试、typecheck 和 `verify:pre-commit`。
- 能独立回滚，不依赖下一阶段才能恢复业务功能。

最终合并前执行：

```bash
pnpm run verify
```

发布前还需按 `docs/development-plan.md` 完成目标 fnOS WebView 实机回归；桌面浏览器验证不能替代 WebView 验证。

## 14. 风险与控制措施

| 风险 | 控制措施 | 停手条件 |
| --- | --- | --- |
| SSE 与 Transition key 耦合 | 只复用 `category-structure` key，字段不入 key | 普通字段更新触发整表过渡 |
| Transition 离场元素影响布局 | 离场 absolute、新内容正常流、pointer-events none | 双滚动条、分页跳位、旧内容可点 |
| 快速分类切换竞态 | 不加 timer；只依赖响应式 key 和 Vue Transition | 最终内容与 active category 不一致 |
| reduced-motion 冻结 loading | 不全局覆盖 animation-duration | spinner 或进行中状态不可辨识 |
| Naive UI 升级私有 DOM 变化 | 不使用 modal 私有选择器；进度直接移除私有依赖 | 方案必须依赖深层私有类 |
| scaleX 视觉失真 | 单组件先验收 1%、50%、100% 和渐变/圆角 | 低百分比明显破损且静态结构无法修正 |
| 长列表合成层过多 | 移除常驻 will-change，不加 JS tween | Layers 或滚动性能明显恶化 |
| 弹窗统一导致行为回归 | 审计优先、逐组件决定、不强迁移 | 焦点、遮罩、loading 保护任一回归 |
| 低价模型扩大范围 | 文件矩阵、提交边界、逐阶段验证 | 准备修改 store/API/后端/依赖 |

## 15. 交给实现模型的工作指令

实现模型一次只领取一个阶段。开始每个阶段前，必须按顺序执行：

1. 阅读 `docs/architecture.md`、`docs/development-plan.md`、`docs/design/DESIGN.md` 和本计划。
2. 查看 `git status --short`，保留并避开用户已有改动。
3. 只打开当前阶段文件和对应测试，不顺手重构相邻组件。
4. 先写或更新当前阶段测试，再做最小生产修改。
5. 完成后运行定向测试、typecheck、`verify:pre-commit`。
6. 搜索当前阶段新增 CSS，确认只补间 transform/opacity，没有 `transition: all`、scale pressed、width/height 动画或常驻 will-change。
7. 查看 diff，逐条对照当前阶段完成标准；未满足则继续修正，不用下一阶段掩盖问题。
8. 报告修改文件、测试结果、人工验收尚未完成的项目和任何偏离计划之处。
9. 明确写出“阶段 N 已完成”后停止，不得自行开始阶段 N+1；由下一次执行指令启动下一阶段。

实现模型不得自行做以下决定：

- 不得跳过阶段 13 的 Figma 批准门禁。
- 不得新增动画库、composable、Pinia 状态或配置项来“方便以后扩展”。
- 不得把所有 `NModal` 迁移到 `AppDialog`。
- 不得把任务列表改成 `TransitionGroup`。
- 不得用随机 key、时间戳、setTimeout 或 requestAnimationFrame 触发动画。
- 不得修改业务 loading、disabled、权限和请求时序来配合视觉效果。
- 不得提交或格式化工作区中与本计划无关的文件。

## 16. 开始实施门禁

只有同时满足以下条件，才可以修改 UI 代码：

1. 用户明确批准本计划。
2. 对应阶段 13 Stitch 母版和具体 Figma frame 已按 `docs/development-plan.md` 获得批准。
3. 实施前重新核对获批 frame 的 hover、pressed、focus-visible、loading 和 reduced-motion 标注。
4. 获批设计与本计划没有冲突；如有冲突，先更新本计划并再次确认。
5. 当前工作区的已有改动已识别，能保证各阶段提交不混入无关文件。

本计划获批只代表允许按阶段实施，不代表可以跳过每个阶段的测试、视觉检查或 fnOS WebView 最终回归。
