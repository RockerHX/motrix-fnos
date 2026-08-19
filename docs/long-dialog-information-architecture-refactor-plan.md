# 长页面弹窗信息架构改造计划

> 状态：阶段 A-C 已完成；阶段 D 自动验证与文档验收已完成，实机验收待执行
> 编制日期：2026-08-18
> 适用范围：Vue 3 + Naive UI 前端辅助弹窗，不改变后端 API、Pinia/service 边界或主任务导航

## 0. 结论先行

当前最需要改造的不是所有页面，而是“多个同级信息域被一次性纵向展开”的辅助弹窗：

| 优先级 | 页面 | 结论 | 目标组件 |
| --- | --- | --- | --- |
| P0 | 设置 `SettingsDialog` | 必须改造。当前包含 5 个独立配置域，且外层 `AppDialog` 把整个 Card 作为滚动面。 | 主导航使用线性 `NTabs`，移动端使用 `NSelect`；常规与 RPC 子导航使用竖直线性 `NTabs` |
| P0 | 诊断 `DiagnosticsDialog` | 必须改造。当前 9 个指标卡 + 3 个大面板连续堆叠，移动端尤其长。 | `NTabs type="line"`，分为概览、连接、日志 |
| P1 | 帮助 `HelpDialog` | 建议一并改造。7 张说明卡没有必要全部展开。 | `NCollapse accordion` + `NCollapseItem` |
| P1 | 关于 `AboutDialog` | 建议一并改造。更新历史是动态长内容，不应与概览和更新检测混在一列。 | `NTabs type="line"`，分为概览、更新记录 |
| 不改 | 新建任务、任务详情、文件确认、调试日志、JSON-RPC 指南、鉴权页、扩展占位页 | 现有结构已经有 Segment、Collapse 或内部滚动，继续拆分会破坏单一流程或增加嵌套导航。 | 保留现状，仅纳入回归验收 |

本计划的目标是让用户在一个弹窗内最多只滚动当前主题面板，而不是把所有 Card 的高度相加。Tab/Segment 只是实现手段；每个页面仍按自身的信息关系选择导航、折叠或内部滚动。

## 1. 审计依据与现状问题

### 1.1 共用弹窗滚动模型

`src/components/ui/AppDialog.css` 当前对 `.app-dialog` 设置了 `max-height` 和 `overflow-y: auto`。因此标题、header action、正文和 footer 都属于同一滚动面；设置与诊断中的每个子面板都会直接增加这个滚动面的高度。

现有断点保持不变：

- 移动端 `< 768px`：弹窗宽度为视口减安全边距，内容高度受 `--app-viewport-height` 限制。
- 窄桌面 `768–1023px`：仍使用桌面外壳，但内容需要允许换行。
- 桌面 `>= 1024px`：可以使用侧向导航和双列指标。

### 1.2 页面级证据

| 页面 | 当前内容 | 长度来源 | 信息关系判断 |
| --- | --- | --- | --- |
| 设置 | `settings-preferences`、`ProxySettings`、`WebAuthSettings`、`JsonRpcTokenSettings`、`LanJsonRpcSettings` | 5 个同级配置域；部分域还有 Alert、表单、操作行和独立确认弹窗 | 互相独立，适合域导航；不应继续堆叠 Card |
| 诊断 | 9 项 `diagnosticMetrics`、`Aria2LogModePanel`、`LogMaintenancePanel`、`EngineStatusPanel` | 指标网格在移动端全部单列；日志维护和引擎面板各自还有指标和按钮 | 可按排障顺序分为概览、连接、日志 |
| 帮助 | 7 个 `AppSectionCard` | 每个说明都独立占一段高度，只有 JSON-RPC 有操作按钮 | 主题之间不互斥，适合可展开列表，不适合 Tab |
| 关于 | Hero、`NDescriptions`、更新检查、指南入口、最近 5 个 changelog entry | changelog 数量随 `CHANGELOG.md` 增长，且每个 entry 含多个 section | 概览和更新记录是两个清晰主题，适合 Tab |
| 调试日志 | 指标、筛选器、日志列表 | `DebugLogList` 自己有 `max-height` 和 `overflow: auto` | 已有内部滚动；外层不再分 Tab |
| 新建任务 | 输入类型 Segment、开始方式 Segment、高级设置 Collapse | 这是一个单一创建流程，切换只影响输入方式；字段必须共同参与一次提交 | 已有合适的 Segment/Collapse |
| 任务详情 | 描述、文件操作、代理开关、技术信息 Collapse | 技术路径已经是按需展开的次要信息 | 保持现状 |
| 文件确认 | `NDataTable`，最大高度 420px | 文件表格本身是独立滚动区域 | 保持现状 |
| JSON-RPC 指南 | 3 个有顺序的步骤、两个端点和复制操作 | 内容是顺序阅读的说明，不是互斥配置域 | 保持顺序，不引入二级 Tab |

### 1.3 需要遵守的项目约束

- 不改变 `src/views/`、Pinia store、service、HTTP/SSE 数据流或后端契约。
- 优先使用 Naive UI；不新增组件库或依赖。
- 业务组件样式继续放在同目录的外部 scoped CSS 文件中。
- 保持深色/浅色主题、中文/英文、键盘焦点、loading/error/disabled/selected 语义。
- 不把 Token 原文、代理 URL 或绝对日志路径带到新的导航标签、标题或日志中。

## 2. 统一弹窗布局方案

### 2.1 一次只保留一个主滚动面

对需要分区的弹窗新增“固定 header/footer、正文单独滚动”的能力，默认行为不变，避免影响未改造的弹窗：

1. 扩展 `AppDialog.vue`，增加可选的 `contentClass` 和 `contentStyle`，透传到 Naive UI `NCard` 的 `content-class` / `content-style`。
2. 在 `AppDialog.css` 增加固定正文模式的基础 class（例如 `app-dialog--fixed-body`）：Card 本身 `overflow: hidden`，content 使用 `display: flex`、`min-height: 0`，header/footer 不随正文滚动。
3. 设置、诊断、帮助、关于按需传入该 class；其他弹窗继续使用现有整卡滚动，降低回归风险。
4. 每个改造弹窗在正文内部只创建一个 `.xxx-pane-scroll` 滚动容器。导航栏、标题、操作栏不放入该滚动容器。
5. 保留 `scrollbar-gutter: stable`、现有 scrollbar token 和 `overscroll-behavior`；禁止页面级横向滚动。

这样实现的结果是：切换分区不会让用户重新滚过其他分区，保存/关闭操作始终可见，移动端软键盘也不会把 footer 推出视口。

### 2.2 Naive UI 组件选型规则

- 同一弹窗内的短标签、互斥视图：使用线性 `NTabs`，保持与关于弹窗一致的切换样式。
- 设置的常规与 RPC 子导航使用 `NTabs type="line" placement="left"`；主导航和诊断使用顶部线性 Tab，不使用 `NMenu`，避免在弹窗中引入路由菜单语义。
- 移动端主分类：`NSelect size="large"`，与同一个 `activeSection` 双向绑定；不用水平滚动的长标签栏承载 4 个以上分类。
- 主题说明且可同时打开/关闭：`NCollapse accordion`。
- 指标继续使用现有 `AppMetricGrid`，不重新造统计卡。
- 所有 `NTabPane` 显式设置 `display-directive="show:lazy"`：首次切换时再挂载，挂载后保留草稿和面板状态，避免默认 `if` 导致切换重置表单。
- `NTabs` 显式提供 `default-value` 或受控 `v-model:value`，避免 Naive UI 默认 slot 推断警告。

## 3. 设置页改造方案（P0）

### 3.1 目标信息架构

```text
设置弹窗
├─ 主导航
│  ├─ 常规
│  ├─ 下载代理
│  ├─ Web 管理安全
│  └─ RPC 访问
│     ├─ 公网反代（127.0.0.1:17081）
│     └─ 局域网入口（17082）
└─ 固定 footer：取消 / 保存
```

桌面主导航使用顶部线性 Tab；移动端使用大号 `NSelect` 选择四个主域。RPC 主域内部使用竖直线性 Tab，避免同时展开公网 Token 和局域网 Token 两个敏感配置域。

### 3.2 分区与现有内容映射

| 分区 | 内容 | 组件与行为 |
| --- | --- | --- |
| 常规 | 按“文件夹授权 / 界面 / 下载配置”拆分：共享文件夹授权与默认下载目录、语言、最大同时下载数和下载/上传限速；移除无实际配置作用的后台驻留提示 | 保留外层 `NForm`、`NAlert`、`NSelect`、`NInputNumber`；继续由 `settingsStore.saveConfig` 一次保存 |
| 下载代理 | `ProxySettings` 的状态、掩码、输入、保存/替换/清除、结果摘要 | 保留子组件自己的 store 和确认弹窗；不并入 `AppConfig` 保存 |
| Web 管理安全 | `WebAuthSettings` 的保护开关、风险提示和修改密码 | 保留 `authStore`、密码校验和独立 `NModal`；敏感输入与当前安全要求不变 |
| RPC 访问 / 公网反代 | `JsonRpcTokenSettings`，含 Token 掩码、生成/显示/保存/清除和指南入口 | 保留专用 Token store；只显示掩码或一次性草稿；指南按钮继续 emit `openRpcGuide` |
| RPC 访问 / 局域网入口 | `LanJsonRpcSettings`，含开关、实际端点、复制、轮换确认和一次性 Token 弹窗 | 保留一次性 Token 清理、手动复制降级和二次确认；不与公网 Token 共用状态 |

### 3.3 设置页交互细节

1. `activeSection` 为局部 UI 状态，弹窗打开时默认回到“常规”；不写入后端、不新增持久化字段。
2. 主导航切换只改变可见 pane，不改变 `form` 草稿；用户可以先修改常规设置，再切到其他分区，最后用固定 footer 的“保存”提交原有 `AppConfig`。
3. Proxy、Web 安全、公网 Token、局域网 Token 的保存继续由各自子组件完成；不要把它们伪装成 footer 的同一次事务。
4. footer 保留现有“取消/保存”语义并始终可见；“保存”只保存常规 `AppConfig`，其他分区的操作按钮继续放在各自面板内。实现时在常规表单附近补充简短说明，避免用户误解。
5. 使用 `display-directive="show:lazy"` 让代理、Token 和局域网面板按首次访问加载；已有 `active` prop 仍以整个设置弹窗的 `show` 为准，关闭弹窗时继续清理敏感草稿和一次性 Token。
6. 切换到 RPC 分区时，状态展示必须仍然包含端点、开关/配置状态和指南入口；任何状态异常都配合文字/图标，不能只靠颜色。
7. 保存、授权、刷新、生成、轮换、复制失败等现有 message/Alert 文案和 loading/disabled 条件全部保留。

### 3.4 设置页文件与实现顺序

1. `src/components/ui/AppDialog.vue`、`src/components/ui/AppDialog.css`：先完成固定正文能力，并补充 props 转发测试。
2. `src/features/settings/components/SettingsDialog.vue`：加入 `activeSection`、桌面/移动导航、四个主 pane，以及常规和 RPC 的竖直二级线性 Tab；删除原来把所有子组件直接串联的布局，不删除子组件。
3. `src/features/settings/components/SettingsDialog.css`：实现导航宽度、pane 最小高度、独立滚动面、移动端 Select、footer 固定和长路径换行。
4. `src/i18n/locales/zh-CN.ts`、`src/i18n/locales/en-US.ts`：增加四个主域、RPC 二级域、当前分区说明和可访问名称。
5. `SettingsDialog.spec.ts`：更新 Naive UI stubs，覆盖默认分区、分区切换、RPC 二级切换、子组件指南事件、footer 保存语义和关闭时状态不变。
6. 现有 `ProxySettings`、`JsonRpcTokenSettings`、`LanJsonRpcSettings`、`WebAuthSettings` 只做必要的 class/ARIA 适配；不重写其 store/service。

## 4. 诊断页改造方案（P0）

### 4.1 目标信息架构

```text
诊断弹窗
├─ 线性 Tab：概览 | 连接 | 日志
├─ 概览：应用/后端/通信指标 + Aria2 引擎状态与操作
├─ 连接：17081 回环反代、17082 局域网入口、两类 Token 状态
└─ 日志：日志模式 + 日志占用/清理 + 调试日志入口
```

三段标签短、互斥且符合排障流程，因此诊断使用顶部线性 `NTabs`，桌面和移动都放在弹窗正文顶部。header action 保持固定：关闭；诊断包导出可继续放在 header，JSON-RPC 指南放在“连接”面板，调试日志放在“日志”面板，避免 header action 过多挤压移动端。

### 4.2 各分区内容

| 分区 | 内容 | 调整原则 |
| --- | --- | --- |
| 概览 | 应用版本、后端状态、通信状态；`EngineStatusPanel` 的 Aria2 配置/进程/RPC 指标、刷新、启动、停止、RPC 检查 | `EngineStatusPanel` 作为进程/RPC 的唯一详细来源；从顶层 `diagnosticMetrics` 移除重复的进程/RPC 卡，但继续保留产品要求的状态文字 |
| 连接 | 回环 `127.0.0.1:17081/jsonrpc`、局域网端点、公共 Token 配置状态、局域网开关/Token 状态、JSON-RPC 指南按钮 | 只展示掩码/配置状态，不展示 Token 原文；`lanJsonRpcStore` 的加载、错误和刷新保持现有语义 |
| 日志 | `Aria2LogModePanel`、`LogMaintenancePanel`、调试日志按钮；诊断包导出后的占用刷新 | 保留 30 分钟详细模式、下次启动生效、80 MiB 警告、引擎运行/未知时禁用清理及二次确认 |

### 4.3 诊断页交互细节

1. `activeSection` 默认“概览”；打开弹窗时仍 emit 一次 `refreshStatus`，不因切换 Tab 重复刷新所有 API。
2. 每个 pane 使用 `display-directive="show:lazy"`。首次进入“连接”才渲染连接指标，首次进入“日志”才渲染日志面板；已挂载 pane 再次切换时保留筛选/倒计时/局部状态。
3. 诊断状态刷新完成后，概览和连接的内容都必须保持可读；不把错误状态压缩成只有红/绿标签。
4. 调试日志仍打开独立 `DebugLogDialog`，不把日志列表复制到诊断主弹窗；日志列表已有内部滚动，保留其过滤器、导出、复制、清空和手动复制降级。
5. 导出诊断包继续只走管理 Session HTTP API；导出成功后调用现有日志占用刷新，不启动或停止 Aria2。
6. EngineStatusPanel 的 `engineStatusUpdated` 事件、启动/停止冲突和 loading/错误反馈保持原有 emit 链路。

### 4.4 诊断页文件与实现顺序

1. `src/features/diagnostics/components/DiagnosticsDialog.vue`：拆出 `activeSection`、三段 pane、动作归属和去重后的指标计算。
2. `src/features/diagnostics/components/DiagnosticsDialog.css`：设置固定正文、线性 Tab 间距、每个 pane 的滚动和移动端 header action 换行。
3. `src/i18n/locales/zh-CN.ts`、`src/i18n/locales/en-US.ts`：增加概览/连接/日志标签和辅助说明。
4. `DiagnosticsDialog.spec.ts`：覆盖默认概览、切换连接后端点与 Token 状态可见、切换日志后面板可见、刷新只在打开时触发、导出/指南/调试日志事件继续发出。
5. 如需减少重复，仅调整 `diagnosticMetrics` 映射和展示，不修改 `EngineStatusPanel` 的服务调用和事件契约。

## 5. 帮助页改造方案（P1）

### 5.1 目标

把 7 个 `AppSectionCard` 改成一个 `NCollapse accordion` 列表，初始只展开第一个核心主题（授权目录），其余主题按需展开。每个 `NCollapseItem` 仍保留：

- 标题和正文说明；
- `enabled` / `pending` / `placeholder` / `troubleshooting` 状态标签，通过 `header-extra` 展示；
- JSON-RPC 条目的“打开配置指南”按钮和原有 `openRpcGuide` emit。

### 5.2 交互与样式

- 使用 Naive UI 的键盘、焦点和展开语义，不自制 accordion。
- `accordion` 保证移动端最多打开一个主题，初始高度可控；正文不再使用完整嵌套 Card。
- 状态标签保留文字，不能只用颜色；触控标题区至少 `44px` 高。
- JSON-RPC 操作按钮在正文中，必要时阻止点击冒泡，避免点击按钮同时收起条目。
- `HelpDialog` 使用固定正文模式，列表本身不再产生第二个页面级滚动面。

### 5.3 文件与测试

- 修改 `src/features/help/components/HelpDialog.vue`、`HelpDialog.css`、中英文 locale。
- 更新 `HelpDialog.spec.ts`：默认展开项、展开/收起、状态标签、指南事件、关闭事件。

## 6. 关于页改造方案（P1）

### 6.1 目标信息架构

```text
关于弹窗
├─ Tab：概览
│  ├─ 应用标识、版本、架构
│  ├─ 维护者、后端、仓库和 Release 链接
│  ├─ 检查更新、版本结果、匹配架构的 FPK 下载入口
│  └─ JSON-RPC 指南入口
└─ Tab：更新记录
   └─ 最近 5 个 changelog entry
```

使用 `NTabs type="line"`，因为概览与更新记录是两个主题，而不是需要高频来回切换的 Segment。`recentChangelogEntries` 的解析逻辑和只显示最近 5 项的规则不变。

### 6.2 交互与文件

- 概览 pane 保留所有现有链接、更新 loading、状态标签和 `openRpcGuide` 行为。
- 更新记录 pane 只移动现有 changelog DOM；不增加搜索、分页或新的候选功能。
- 使用 `display-directive="show:lazy"`，首次打开更新记录时才创建长列表；已打开后切回不重置阅读位置。
- 修改 `AboutDialog.vue`、`AboutDialog.css`、中英文 locale，并更新 `AboutDialog.spec.ts` 的 Tab 切换和 changelog 可见性断言。

## 7. 明确不改造的页面

| 页面 | 不改原因 | 仅需验证 |
| --- | --- | --- |
| `TaskCreateDialog` | 已有输入类型 Segment、开始方式 Segment、高级设置 Collapse；再拆会割裂一次提交流程 | 三种输入、校验、提交 loading、移动端滚动和高级设置状态 |
| `TaskDetailsDialog` | 技术信息已经按需 Collapse；主详情和操作必须同时可见 | 长路径换行、代理确认、文件操作和关闭按钮 |
| `TaskFileConfirmDialog` | 文件列表是明确的数据表格，已有 `max-height: 420px` 内部滚动 | 大量文件、移动端表格可读性、至少选择一项校验 |
| `DebugLogDialog` | `DebugLogList` 已有独立 `max-height`/`overflow`；筛选器和日志列表是一个连续工具 | 筛选、空态、刷新、复制/下载、清空和手动复制降级 |
| `JsonRpcGuideDialog` | 三步内容有先后顺序；改成 Tab 会隐藏前置安全说明 | 三步顺序、端点复制失败降级、打开设置事件 |
| `AuthGate` / `WebAuthSettings` 的独立密码弹窗 | 鉴权流程是安全门禁，不属于普通设置导航 | 首次初始化、登录、Session 失效、密码焦点和移动软键盘 |
| `ExtensionsPlaceholder` | 内容极短且为占位说明 | 不出现假按钮或新增候选功能 |

所有 `AppConfirmDialog`、删除确认、Token 清理/轮换确认和 `DebugLogManualCopyDialog` 都是单一动作的小弹窗，不纳入本次信息架构拆分；只随所属页面做回归验证。

## 8. 状态、数据流与安全要求

### 8.1 UI 状态

- `activeSection`、RPC 二级 `activeRpcSection`、About tab 和 Help 展开项都是局部 UI 状态，不进入 Pinia 或 SQLite。
- `NTabPane display-directive="show:lazy"` 只控制挂载和可见性，不改变业务数据来源。
- 不因 Tab 切换重复调用 `loadConfig`、`loadAccessiblePaths`、诊断刷新或写 API；首次访问的子面板按现有 `active`/store 规则读取状态。

### 8.2 保存与关闭

- 常规设置仍由 `settingsStore.saveConfig` 保存，Proxy/Token/Web 安全/局域网入口继续由各自 store/service 保存。
- 现有关闭禁用、mask-closable、loading 和失败后保留输入语义不变。
- 关闭设置弹窗时继续清理代理草稿、公共 Token 草稿、局域网一次性 Token 和密码表单；切换分区不能让敏感值进入 URL、日志或持久化。
- 诊断导出、日志清理、日志模式切换仍只通过管理 Session API，不给外部 JSON-RPC 增加入口。

### 8.3 数据流边界

```text
导航状态（组件 ref）
        ↓ 只决定显示哪个 pane
现有 Vue 组件
        ↓ 继续调用原有 Pinia store/service
HTTP client / SSE / Rust server
```

不新增后端路由、不新增数据库迁移、不复制一套移动端业务状态。

## 9. 响应式与视觉实现要求

### 9.1 视口行为

至少验证：`390×844`、`1024×768`、`1440×900`，并分别覆盖深色/浅色、中英文。

- `390px`：设置主导航换成 `NSelect size="large"`；RPC 二级线性 Tab 使用竖直导航；按钮和输入宽度 100%；正文单列；不出现页面级横向滚动。
- `1024px`：设置保留顶部主导航和竖直子导航，导航宽度固定在可读范围；内容 pane 允许长路径和英文标签换行。
- `1440px`：设置主导航、子导航和内容区充分利用宽度；诊断线性 Tab 和指标网格充分利用宽度，但不恢复所有面板的纵向展开。
- 弹窗 header/footer 固定，正文 pane `min-height: 0`；所有动态内容只能扩大当前 pane 的滚动高度。

### 9.2 设计系统约束

- 继续使用现有深海军蓝/Logo 蓝 token、细边框和紧凑圆角；不新增渐变、玻璃拟态、厚阴影、营销 Hero 或嵌套 Card。
- 新导航只使用 Naive UI 标准语义；需要图标时通过现有 `AppIcon`/Tabler 适配层，不直接引入新图标库。
- 正文不小于 `14px`，触控目标至少 `44×44px`，焦点环清晰；状态同时显示文字/图标。
- 动效仅保留 Naive UI 的轻量切换，尊重 `prefers-reduced-motion`；不添加列表瀑布或持续动画。

## 10. 测试与验收清单

### 10.1 单元测试

| 测试文件 | 必测断言 |
| --- | --- |
| `src/components/ui/AppDialog.spec.ts` | `contentClass/contentStyle` 正确透传；默认弹窗行为不变 |
| `SettingsDialog.spec.ts` | 默认常规、切换 4 个主域、RPC 二级切换、footer 保存、指南 emit、关闭事件、子面板只在首次访问挂载 |
| `DiagnosticsDialog.spec.ts` | 概览/连接/日志切换、状态刷新只在打开时触发、引擎事件、导出/指南/调试日志事件、错误/空态仍显示 |
| `HelpDialog.spec.ts` | 初始展开项、accordion 展开/收起、状态标签、指南按钮、关闭 |
| `AboutDialog.spec.ts` | 概览默认显示、更新记录切换、更新操作、RPC 指南、关闭 |
| 现有子组件 specs | Proxy、Token、局域网 Token、密码、日志清理的原有行为保持通过 |

### 10.2 静态与构建检查

按项目脚本执行：

1. `pnpm typecheck`
2. `pnpm test:unit`
3. `pnpm build`

不新增依赖；若测试需要 Naive UI stub，集中更新对应 `*.spec.ts`，不在生产 `.vue/.ts` 文件中写测试逻辑。

### 10.3 手工视觉验收

- 每个改造弹窗打开后，header 标题、关闭按钮和 footer 不随正文滚走。
- 设置在常规、代理、Web 安全、RPC 公网、RPC 局域网之间切换，表单草稿、loading、错误和敏感值清理符合预期。
- 诊断在三个线性 Tab 间切换，连接端点和日志占用不会被概览内容推到数屏之外；日志列表仍只在自身容器滚动。
- 帮助默认不会一次性展示 7 段正文；关于默认不会一次性展示最近 5 个版本的全部内容。
- 中文/英文长标签、长 URL、长路径、空数据、API 失败、按钮 loading、禁用清理和运行中引擎均无重叠或横向溢出。
- 关键操作可用键盘完成，Tab 顺序与视觉顺序一致；移动端触控目标不小于 `44×44px`。

## 11. 实施顺序与完成定义

### 阶段 A：弹窗基础能力

- 完成 `AppDialog` content props 和固定正文 class。
- 先在一个最小示例或现有测试中验证 header/footer 固定、正文滚动和移动高度。

完成定义：未改造弹窗视觉和滚动行为不变，新增能力有单元测试。

### 阶段 B：设置与诊断（P0）

- 先改设置，再改诊断；两页共用弹窗基础能力但不共用业务状态。
- 同步 locale、spec 和 CSS；每页完成一次桌面/移动手工检查。

完成定义：设置和诊断不再把所有同级 Card 作为一个连续滚动面，所有原有操作和事件通过测试。

### 阶段 C：帮助与关于（P1）

- 帮助先替换为 Collapse，再把关于拆为概览/更新记录。
- 只做信息收纳，不新增搜索、分页、通知或其他候选功能。

完成定义：辅助页初始内容高度明显降低，原有状态标签、链接和事件不变。

### 阶段 D：全量验证

- 执行 typecheck、unit test、build。
- 完成三视口、双主题、双语言和状态矩阵验收。
- 将实现结果和任何偏离本计划的交互决策回写到本文件，必要时再同步 `docs/design/ui-product-requirements.md` 的 `[现状]` 描述。

#### 当前验收记录（2026-08-19）

- 自动验证：`pnpm run typecheck`、`pnpm run test:unit`（98 个文件、422 条测试）、`pnpm run build`、`git diff --check` 已通过。
- 文档验收：阶段 A-C 的实际组件选型、设置常规三级内容划分、滚动模型和阶段 D 状态已回写本文件。
- 已完成的实现偏差记录：设置和诊断统一采用关于页风格的线性 Tab；设置常规与 RPC 子导航采用竖直线性 Tab；移动端设置主导航采用 `NSelect`；后台驻留提示已移除。
- 已修复：移动端设置主导航的隐藏 CSS 不再误隐藏常规和 RPC 二级导航。
- 待执行实机验收：390×844、1024×768、1440×900；中英文；深色/浅色；ARM fnOS WebView 的滚动、导航、固定 header/footer 和横向溢出检查。

## 12. 非目标与风险

### 非目标

- 不改变主侧栏/底部导航、任务分类或 URL 路由。
- 不把设置改造成独立路由页面，不新增后端持久化的 tab 偏好。
- 不把所有 Card 强行改为 Segment；不新增“全部任务”“搜索”“详情抽屉”等候选功能。
- 不改变 Token、代理、鉴权和日志的安全边界。

### 风险与处理

| 风险 | 处理 |
| --- | --- |
| Tab 默认 `if` 导致子组件重新挂载、草稿丢失 | 统一使用 `display-directive="show:lazy"`，并测试切换前后状态 |
| footer 仍被正文滚走 | 先完成 `AppDialog` fixed-body 能力，再迁移业务页；用 390/1024/1440 三视口验证 |
| 移动端主导航标签过长 | 设置使用大号 `NSelect`；诊断使用三个短标签的顶部线性 Tab；不让标签栏横向溢出 |
| 独立保存和全局保存被用户混淆 | 保留各子组件操作区，在常规表单附近明确 footer 只保存 `AppConfig` |
| 诊断指标去重后状态来源不清 | 让 `EngineStatusPanel` 作为 Aria2 详细状态唯一来源，保留其事件和刷新契约，并增加测试 |
| 主题/语言切换后布局变形 | 在中英文、深浅色及长语义路径验收中检查换行和最小宽度 |
