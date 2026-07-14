# 交互动画完善执行计划

> 状态：待评审，未开始实施  
> 编写日期：2026-07-14  
> 适用范围：阶段 13 UI 重设计中的交互反馈与状态过渡

## 1. 结论

有必要完善交互动画，但目标不是增加装饰性动效，而是补齐操作反馈、状态变化的连续性和界面层级切换提示。

当前界面已经具备完整业务交互和 Naive UI 默认弹窗、按钮 loading 等基础反馈，但自定义界面的动效规则尚未形成闭环：

- 导航、工具栏、移动端浮动创建按钮的 hover、pressed、disabled 状态变化大多瞬时发生。
- 分类切换会直接替换空状态、任务列表或扩展页面，缺少短暂且可感知的上下文切换反馈。
- 桌面任务操作区已声明 `var(--app-transition-fast)`，但全局 token 未定义，该过渡实际不完整。
- 任务进度已有 `360ms` CSS transition，可避免 SSE 数值更新生硬跳变；但当前动画属性是 Naive UI 内部的 `max-width`，不符合 `docs/design/DESIGN.md` 中“只动画 `transform` 和 `opacity`”的约束。
- 尚未实现全局 `prefers-reduced-motion` 降级策略。

因此建议做一轮小范围、系统化的交互动效治理。预期收益是操作反馈更明确、页面切换更连贯、实时进度更稳定；不改变信息架构、业务流程或数据状态模型。

## 2. 设计与实现原则

实施时严格遵守现有设计文档：

1. 常规状态过渡使用 `120–180ms`，弹窗进入/退出不超过 `200ms`，统一使用 ease-out。
2. 仅动画 `transform` 和 `opacity`；颜色、背景和边框的短过渡只作为控件状态反馈，不触发布局与重排。
3. pressed 只允许 `translateY(1px)`，不缩放任务卡片或按钮。
4. 不使用弹簧、视差、列表瀑布入场、持续循环动画或页面滚动动画。
5. SSE 刷新不触发任务项入场、退场或排序动画；实时数据更新不得闪烁。
6. `prefers-reduced-motion: reduce` 下关闭非必要位移、内容切换和进度补间，并将反馈保留为即时状态变化。
7. 不引入动画库，继续使用 Vue Transition、CSS 与 Naive UI 现有能力。

## 3. 实施范围

### 3.1 动效 token 与无障碍基线

修改文件：

- `src/styles/tokens.css`
- `src/styles/base.css`
- `src/app/providers/NaiveProvider.vue`（仅在 Naive UI 主题变量确有可控入口时修改）

执行内容：

- 新增快速、常规和弹窗三档时长 token，以及统一 ease-out token。
- 修复 `--app-transition-fast` 缺失问题，禁止组件继续散落硬编码常规时长。
- 增加全局 reduced-motion 规则；不通过 `transition: all` 粗暴覆盖组件。
- 核对 Naive UI modal、message、button 的默认时长；能通过正式 theme override 对齐则统一，不能则保留组件默认值，避免依赖内部类名。

完成标准：自定义组件均使用统一 token；系统减少动态效果时不存在非必要位移或缓动。

### 3.2 高频控件反馈

修改文件：

- `src/layouts/SidebarNav.vue`
- `src/layouts/Topbar.vue`
- `src/views/MainWindow.vue`
- `src/features/tasks/components/TaskActions.vue`
- 其他在阶段 13 获批设计中使用原生 button 的同类组件

执行内容：

- 为导航项、顶部工具按钮、任务操作按钮和移动端浮动创建按钮统一 hover、focus-visible、pressed、disabled 过渡。
- pressed 使用最多 `translateY(1px)`；hover 只改变颜色、背景、边框或透明度，不改变控件尺寸。
- 为浮动创建按钮的显示/隐藏增加短暂 opacity/translate 过渡，仅由分类切换或运行退出状态触发。
- loading 继续使用 Naive UI 局部 loading，不新增持续旋转或脉冲动画。

完成标准：鼠标、键盘和触控操作都有明确且尺寸稳定的反馈；连续点击不会造成按钮位移残留。

### 3.3 分类与内容状态切换

修改文件：

- `src/views/MainWindow.vue`
- `src/layouts/Topbar.vue`
- 必要时补充 `src/features/tasks/composables/useTaskCategoryView.ts` 的纯 UI 切换标识，不新增持久状态

执行内容：

- 使用 Vue Transition 为用户主动分类切换增加 `120–160ms` 的 opacity 与最多 `4px` 位移。
- 空状态与列表之间的切换只在结构状态改变时播放一次，不在 SSE 更新现有任务字段时播放。
- 顶部分类标题与主内容保持同一节奏，避免标题先跳、内容后换。
- 不使用 `TransitionGroup` 包裹任务列表，不为新增、删除、排序和分页结果增加逐项动画。

完成标准：分类切换可感知但不拖慢操作；SSE 连续刷新、任务进度变化和状态标签变化不触发整页动画。

### 3.4 进度动画合规化

修改文件：

- `src/features/tasks/components/TaskProgressBar.vue`
- `src/features/tasks/components/TaskProgressCell.vue`
- 对应 `*.spec.ts`

执行内容：

- 保留“新进度不得因旧事件倒退”的现有逻辑。
- 将进度补间从 `max-width` 迁移为 transform 驱动；若 Naive UI 公共 API 无法满足，则先提交最小技术验证，由获批设计确认是否保留 NProgress 或使用等价的可访问进度轨道。
- 进度时长允许独立于常规控件，以覆盖 SSE 刷新间隔，但不得产生持续动画；reduced-motion 下即时更新。
- 移除不再需要的 `will-change`，或只在实际过渡期间启用，避免大量任务常驻合成层。

完成标准：进度视觉连续、不会倒退、不会常驻运行；长任务列表下无明显滚动掉帧。

### 3.5 弹窗与消息反馈核对

修改文件：

- `src/components/ui/AppDialog.vue`
- 仍直接使用 `NModal` 的任务详情、创建任务和调试日志等组件

执行内容：

- 统一检查所有弹窗的进入/退出时长、遮罩同步、关闭焦点恢复与 loading 期间关闭保护。
- 优先复用 Naive UI 默认 transition；只有与 `200ms` 上限或 reduced-motion 明显冲突时才通过公共配置修正。
- 不为弹窗内容增加分段、瀑布或 stagger 动画。

完成标准：各弹窗节奏一致，遮罩与内容无脱节，键盘焦点和 loading 行为不因动画回归。

## 4. 明确不做

- 不新增页面首屏入场动画、任务卡片逐个入场、数字滚动、骨架闪烁或成功庆祝动画。
- 不对 SSE 引发的任务状态、速度、剩余时间和标签更新添加淡入淡出。
- 不为列表删除添加高度收缩动画，避免布局重排和批量操作抖动。
- 不修改业务 store、HTTP/SSE 协议、SQLite 或后端实现。
- 不引入 motion/GSAP 等依赖，也不覆盖 Naive UI 私有实现细节。
- 不在阶段 13 已批准 frame 之外自行改变布局、颜色或组件结构。

## 5. 执行顺序与提交边界

### 批次 A：基础规则与高频反馈

1. 建立 motion token 和 reduced-motion 基线。
2. 完成导航、工具栏、任务操作和浮动创建按钮反馈。
3. 修复未定义 transition token。

验证：单元测试、类型检查、三个目标视口的鼠标/键盘/触控检查。

### 批次 B：上下文切换

1. 完成分类内容切换与顶部标题同步。
2. 验证空状态、列表、扩展、回收站和分页组合。
3. 使用模拟 SSE 连续更新确认不会重复触发入场动画。

验证：组件测试与开发环境连续更新观察。

### 批次 C：进度与弹窗治理

1. 完成进度条 transform 技术验证与实现。
2. 核对统一弹窗及仍直接使用 NModal 的组件。
3. 完成 reduced-motion 全流程回归和性能检查。

验证：长任务列表、低速/高速进度变化、批量操作、弹窗 loading 与关闭流程。

每个批次独立提交，任一批次出现体验或性能回归时可单独回滚。

## 6. 测试与验收矩阵

自动化验证：

- 为内容 Transition 的触发条件补充组件测试，证明分类切换触发、普通 SSE 字段更新不触发。
- 为进度组件保留数值归一化、不倒退、任务切换重置和完成态测试。
- 为 reduced-motion 使用稳定 class 或状态契约时补充测试；纯 CSS 媒体查询通过浏览器人工验收，不编写依赖实现细节的脆弱断言。
- 每批执行 `pnpm run verify:pre-commit`，最终执行 `pnpm run verify`。

视觉与交互验收：

| 维度 | 验收场景 |
| --- | --- |
| 视口 | `390x844`、`1024x768`、`1440x900` |
| 主题与语言 | 深色/浅色，中/英文；以阶段 13 实际已实现组合为准 |
| 输入方式 | 鼠标 hover/pressed、键盘 focus-visible/Enter/Space、移动端触控 |
| 内容状态 | Loading、Empty、Error、Disabled、Selected、Runtime exiting |
| 实时更新 | 多任务 SSE 连续刷新、进度增长、状态切换、任务完成 |
| 降级 | 操作系统 reduced-motion 开启后无非必要移动和补间 |
| 性能 | 长列表滚动无明显掉帧，空闲时无持续 animation 或计时循环 |

## 7. 风险与控制

- **SSE 与 Transition key 耦合**：只允许分类/结构视图 key 驱动内容过渡，任务字段不得进入 key。
- **Naive UI 私有 DOM 变更**：优先使用公共主题配置和组件 API；不得依赖可避免的深层内部选择器。
- **进度条迁移视觉差异**：先做单组件验证，确认渐变、圆角、完成态和未知总大小后再替换。
- **低性能 WebView**：限制为 transform/opacity，禁止大面积模糊与常驻 `will-change`，在 fnOS WebView 做最终实机回归。
- **动画掩盖操作延迟**：业务 loading 和禁用状态保持即时生效，动画不得延后请求、状态提交或错误显示。

## 8. 开始实施的门禁

满足以下条件后才开始修改 UI 代码：

1. 用户批准本计划。
2. 对应阶段 13 Figma frame 已按 `docs/development-plan.md` 的门禁获得批准。
3. 实施前再次核对获批 frame 的 hover、pressed、focus-visible、loading 和 reduced-motion 标注；若与本计划冲突，先更新本文档并重新确认。

