# Design System: Motrix fnOS

> 状态：待用户确认的 UI 视觉系统  
> 决策基线：`1A 2B 3B 4B 5A`（2026-07-12）  
> 实现边界：Vue 3 + Naive UI；不得以视觉稿改变业务架构。

## 1. Visual Theme & Atmosphere

Motrix fnOS 是安静、可靠、紧凑的原生工具型界面，而非营销网站或通用 SaaS Dashboard。

- **密度 8/10**：优先快速扫描任务名称、进度、速度、ETA 和错误。
- **布局变化 3/10**：稳定、可预测；信息对齐优先于装饰性不对称。
- **动效 2/10**：仅为状态变化、弹窗和直接操作提供反馈。
- **视觉特征**：深炭灰或柔和浅灰中性色，单一克制绿色强调，细边框、少阴影、紧凑圆角。
- **禁止**：Hero、宣传区、滚动叙事、玻璃拟态、装饰渐变、发光、噪点纹理、嵌套卡片、大圆角 SaaS 风格和持续循环动画。

## 2. Color Palette & Roles

颜色以语义 token 使用，不在组件内直接写主题色。绿色是唯一品牌强调色；危险、警告、信息色仅表达状态。

### 2.1 Dark theme（默认）

| Token | Value | Role |
| --- | --- | --- |
| `--color-shell` | `#111513` | 应用外壳与侧栏底色 |
| `--color-canvas` | `#151917` | 主内容画布 |
| `--color-surface` | `#1B201D` | 弹窗、输入、任务边界 |
| `--color-surface-raised` | `#222824` | hover、选中与浮层 |
| `--color-border` | `#343C37` | 结构边框 |
| `--color-border-subtle` | `#29302C` | 分隔线和弱边框 |
| `--color-text-primary` | `#EEF3EF` | 主标题和关键数值 |
| `--color-text-secondary` | `#B6C0B9` | 正文和次要字段 |
| `--color-text-muted` | `#7F8B83` | 辅助说明和占位符 |
| `--color-accent` | `#68AE5A` | 主操作、进度和焦点 |
| `--color-accent-hover` | `#7ABE6C` | 强调色 hover |
| `--color-accent-pressed` | `#57964B` | 强调色 pressed |
| `--color-accent-soft` | `#253626` | 选中背景 |

### 2.2 Light theme

| Token | Value | Role |
| --- | --- | --- |
| `--color-shell` | `#EEF1EE` | 应用外壳与侧栏底色 |
| `--color-canvas` | `#F7F9F7` | 主内容画布 |
| `--color-surface` | `#FFFFFF` | 弹窗、输入、任务边界 |
| `--color-surface-raised` | `#F0F4F0` | hover、选中与浮层 |
| `--color-border` | `#CBD3CD` | 结构边框 |
| `--color-border-subtle` | `#DDE3DE` | 分隔线和弱边框 |
| `--color-text-primary` | `#19201B` | 主标题和关键数值 |
| `--color-text-secondary` | `#48534B` | 正文和次要字段 |
| `--color-text-muted` | `#707C73` | 辅助说明和占位符 |
| `--color-accent` | `#4F9145` | 主操作、进度和焦点 |
| `--color-accent-hover` | `#43813A` | 强调色 hover |
| `--color-accent-pressed` | `#376E30` | 强调色 pressed |
| `--color-accent-soft` | `#E4F0E2` | 选中背景 |

### 2.3 Shared status colors

| Semantic role | Dark | Light | Usage |
| --- | --- | --- | --- |
| Success | `#68AE5A` | `#4F9145` | 完成、连接正常 |
| Warning | `#D5A64A` | `#9B6C12` | 等待确认、待实现 |
| Danger | `#D56A6A` | `#B64040` | 错误、删除 |
| Info | `#6F9FC7` | `#39749E` | 诊断说明、解析中 |

状态必须同时使用文字或图标；禁止只靠颜色表达。焦点环使用主题强调色并提供至少 `2px` 可见轮廓。

## 3. Typography Rules

- **界面字体**：`-apple-system, BlinkMacSystemFont, "Segoe UI", "PingFang SC", "Microsoft YaHei", sans-serif`。不下载或新增字体包。
- **数字字体**：继承系统字体并启用 `font-variant-numeric: tabular-nums`；GID、token 和技术日志可使用系统等宽栈。
- **页面标题**：`20px / 28px / 600`；弹窗标题 `18px / 26px / 600`。
- **任务名称**：桌面 `15px / 22px / 600`，移动 `15px / 22px / 600`。
- **正文**：`14px / 21px / 400`；辅助信息最小 `12px / 18px`。
- 禁止全大写标签、超大展示标题、装饰性字距；中英文切换不得改变层级。

## 4. Spacing, Radius, Border & Shadow

- **4px 基线**：`4, 8, 12, 16, 20, 24, 32px`；任务内部常用 `8–12px`，页面 gutter 桌面 `24px`、移动 `14–16px`。
- **圆角**：输入/按钮 `6px`，任务与小面板 `8px`，弹窗 `10px`；胶囊形只用于有语义的状态标签。
- **边框**：默认 `1px`；优先以边框和间距区分层级，不堆叠多层背景卡片。
- **阴影**：只用于弹窗和确有悬浮层级的菜单；任务列表在桌面不使用常驻厚阴影。
- **层级**：画布 → 单一任务边界 → 必要的内部分隔；禁止卡片内再嵌套完整卡片。

## 5. Component Stylings

- **导航**：当前项使用浅强调背景、强调色图标和高对比文字；hover 不改变布局尺寸。
- **工具栏**：主创建按钮使用强调色，批量操作使用次级/图标按钮；禁用原因可通过 title 或邻近状态理解。
- **任务项**：名称和状态优先，进度与数值对齐；错误就近呈现；操作在桌面稳定靠右，在移动端形成清晰触控区。
- **按钮**：hover 改变背景或边框，pressed 仅 `translateY(1px)`；危险操作不使用品牌绿。
- **输入**：标签在上、帮助或错误在下；focus、error、disabled 和 loading 状态尺寸稳定。
- **弹窗**：标题、可选 header action、可滚动内容和固定操作区；移动端使用可视高度并避让安全区。
- **状态标签**：紧凑圆角矩形，不使用过量 pill；包含文字，不以圆点颜色单独表达。
- **进度条**：细而清晰，完成度、百分比和速度信息不相互覆盖；未知总大小有明确占位语义。
- **空状态**：使用简单线性图形或现有图标、简短说明与真实下一步；禁止大型装饰插画。
- **加载**：按钮操作使用局部 loading；内容首次加载可使用匹配任务/指标形状的骨架，避免全屏 spinner。
- **错误**：表单内联错误优先，跨区域操作使用现有 message；不使用“Oops”或感叹号文案。

## 6. Layout Principles

- 桌面保持左侧导航、顶部工具栏和主内容区；不得改为顶部网站导航或营销 Dashboard。
- 桌面任务区使用高密度纵向列表，宽屏内容仍充分利用可用空间，不强制营销站式窄容器。
- 窄桌面保持相同信息架构，压缩留白并允许次要字段合理换行或收纳。
- 移动端切换为单列任务卡片与底部导航；业务操作、分类语义和数据来源不变。
- 长文件名、URL、路径以可预测的省略或换行处理；关键操作永不被文本推出视口。
- 禁止页面级横向滚动、绝对定位堆叠、重叠文本和依赖固定高度容纳动态内容。

## 7. Responsive Rules

- **Mobile `< 768px`**：单列、底部导航、`14–16px` gutter，弹窗接近全宽，触控目标至少 `44px`。
- **Compact desktop `768–1023px`**：保留桌面外壳语义，减少 gutter，任务元数据允许重排。
- **Desktop `>= 1024px`**：固定侧栏和工具栏，任务信息横向组织。
- 设计验收至少覆盖 `390×844`、`1024×768`、`1440×900`。
- 所有主题和语言组合均不得横向溢出；键盘焦点顺序与视觉阅读顺序一致。

## 8. Motion & Interaction

- 常规状态变化 `120–180ms`，弹窗进入/退出不超过 `200ms`；使用标准 ease-out。
- 只动画 `transform` 和 `opacity`；禁止弹簧编舞、列表瀑布入场、视差和持续循环微动效。
- pressed 位移不超过 `1px`，hover 不缩放任务卡片。
- `prefers-reduced-motion: reduce` 时关闭非必要动画。
- SSE 数据刷新不得让任务项闪烁、重排抖动或反复播放进入动画。

## 9. Theme Behavior

- 主题选项仅为 `dark` 和 `light`，默认 `dark`。
- 主题选择属于长期 UI 偏好；代码实施时扩展现有 `/api/ui-preferences` 数据，不新建另一套持久化接口。
- 首屏应在应用数据可用前使用默认深色，加载偏好后稳定切换，避免反复闪烁。
- Naive UI theme 与应用 CSS token 必须由同一主题状态驱动。
- 两个主题具有相同信息结构、尺寸、状态色语义和交互能力。

## 10. Anti-Patterns (Banned)

- 不新增搜索、右键菜单、底部状态栏、做种分类、任务详情抽屉等候选功能。
- 不使用 Hero、宣传文案、玻璃拟态、霓虹发光、紫蓝渐变、噪点、浮动装饰物。
- 不使用纯黑 `#000000`、纯白大面积高亮、过饱和绿色或多品牌强调色。
- 不使用超大圆角、嵌套卡片、等宽三卡片营销布局或无意义徽标。
- 不引入新动画库、字体包、图标库或非标准 canvas 控件。
- 不用静态假按钮表现未实现功能；候选需求只能出现在独立说明区。
