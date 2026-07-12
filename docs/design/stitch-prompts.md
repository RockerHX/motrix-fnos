# Motrix fnOS Google Stitch 提示词

> 状态：已确认，待通过 Stitch MCP 执行单页 PoC  
> 决策基线：`1A 2B 3B 4B 5A`（2026-07-12）  
> 使用方式：先完成第 0 节的设计系统 PoC。不要把每个页面当成独立风格探索；设计系统创建并验证后，后续提示词只描述页面结构、内容和状态。第 2 节仅作为后续需求规格，不直接整段提交。

## 0. 在 Stitch 当前界面中的使用方法

### 0.1 创建项目与设计系统（MCP 优先）

以下是有来源支撑的流程；其中 MCP 工具名和请求形状来自 Google Labs `stitch-skills` 参考实现，不代表 Stitch 网页 UI 一定提供同名控件：

1. 若已配置 Stitch MCP，先查找或创建项目，再检查项目是否已有 Design System。
2. 将仓库的 `docs/design/DESIGN.md` 作为输入上传，并创建项目级 Design System；创建前必须获得用户确认。应用到屏幕时只传递真实 screen instance 的 `id` 和 `sourceScreen`。
3. 只选择 **Web**。依据 `docs/architecture.md`，Motrix 的桌面浏览器、手机浏览器和 fnOS App WebView 共用同一套 Vue Web UI。
4. 如果只能使用网页 UI，先用当前界面完成同等的设计系统导入，并截图记录控件；在导入方式未实证前，不把“以您的设计为基础”后的下一层控件写成确定步骤。

`docs/design/DESIGN.md` 是仓库唯一视觉规范。它现在同时满足 Labs skill 要求的 YAML front matter 和 Markdown 规则正文；仍需通过一个桌面页面验证 Stitch 是否实际应用 token。不要选择 Alexandria、Bauhaus、Glacier、Carbon 等预设，因为它们不是仓库已确认的设计系统。

### 0.2 生成纪律

- 每次只生成 **一个 Web 页面、一个主题、一个运行状态**。
- 第一张桌面任务页最多生成 2 个候选；选定基准后不再生成新的风格候选。
- 生成后续状态时，在已批准页面上使用 Stitch 的编辑/迭代入口，并明确要求保持布局、组件、颜色、字体和圆角不变。
- 不要求用户重命名项目或页面；用“截图文件夹名 + 生成顺序”记录即可。
- 默认产品页面不混入 hover、focus、pressed、disabled 状态板；交互状态后续单独生成。
- 出现错误的字体、颜色、断点、页面外壳或未实现功能时停止并交给 Codex 判断；用户不负责修提示词。

### 0.3 现在只提交的首个页面提示词

本提示词只在项目级 Design System 创建成功后由 Codex 通过 Stitch MCP 提交。不得在没有 Design System 的项目中直接生成，也不得与第 1 节全局上下文重复提交视觉 token。

```text
Create one responsive Web UI screen for Motrix fnOS: the desktop Downloading screen in the default dark theme. Target layout: 1440x900 desktop viewport. Generate only the normal runtime state, not a state sheet.

Use a compact fixed left navigation, a compact top toolbar, and a dense vertical list of horizontal task items. Show the shared active, paused, error, magnet-resolving, and file-confirmation samples from the global context. Preserve every real status and only the valid actions. Prioritize task name and textual status, followed by progress, downloaded/total size, speed, ETA, category, error and contextual actions.

Follow the currently selected custom DESIGN.md exactly. Do not create another design system or substitute a Stitch preset. Do not show hover, focus, pressed, loading or disabled demonstrations in this screen. Do not add Search, System theme, count badges, a bottom speed bar, a right detail drawer, seeding categories, Purge, Export Logs, or any other unapproved feature. Return one desktop screen only.
```

生成后立即停止。不要继续移动端、浅色主题、弹窗或其他状态；Codex 先通过 MCP 读取完整截图和 HTML，并记录评审结论。

## 1. 全局上下文

```text
Design a production-grade download manager for fnOS named Motrix. It is a frequently used utility inside desktop web, mobile browser, and fnOS app WebView environments. It is not a marketing website and not a generic SaaS dashboard.

Preserve the existing information architecture and workflows. Desktop uses a left navigation, top task toolbar, and main task content. Mobile uses one-column task cards and bottom navigation. The navigation categories are Downloading, Completed, Stopped, Trash, and Extensions. Auxiliary entries open Settings, Help, About, and Diagnostics. Do not redesign these relationships.

Create equivalent dark and light themes. Dark is the default. Only offer Dark and Light, not System. Follow the supplied DESIGN.md exactly for semantic colors, system font stack, spacing, radius, state colors, breakpoints, and motion. The atmosphere is native to fnOS: calm, reliable, compact, high-density, and low-motion. Use one restrained green accent. Use tabular figures for progress, size, speed, and ETA.

Prioritize task name, textual status, progress, downloaded and total size, speed, ETA, category, error, and available actions. Use thin structural borders and spacing instead of nested cards or heavy shadows. All status colors require text or icon support. All desktop controls need hover, pressed, focus-visible, disabled, and loading states. Touch targets are at least 44 by 44 pixels.

The UI must be realistically implementable with Vue 3 and Naive UI. Do not invent a separate business state, custom canvas controls, or visual behaviors that require new animation libraries. Use Chinese primary copy with layouts that also fit English.

Never add search, context menus, a bottom global speed bar, seeding or all-tasks categories, category count badges, a task detail drawer, cloud accounts, collaboration, analytics, editable Aria2 internals, notification settings, or any fake clickable feature. Never use hero sections, marketing copy, glassmorphism, decorative gradients, neon glow, noise textures, floating ornaments, nested cards, oversized headings, excessive pills, or large SaaS-style radii.

Use realistic shared sample data across every screen:
- active: ubuntu-24.04.2-desktop-amd64.iso, 3.1 GB / 5.8 GB, 53.4%, 12.7 MB/s, ETA 3m 39s, category 默认
- paused: fnos-backup-2026-07-11.tar.zst, 847 MB / 2.4 GB, 34.5%
- error: documentary-episode-07.mkv, 1.8 GB / 4.6 GB, error text 网络连接中断
- magnet resolving: 开源纪录片合集, status 正在解析磁链
- confirmation: Linux 教程合集, status 等待选择文件
- completed: aria2-next-linux-x86_64.tar.gz, 18.7 MB / 18.7 MB, 100%
- authorized directory: /vol1/downloads
```

## 2. 页面提示词

> 本节从首轮审计后降级为“页面需求规格”。只有第 0 节锚点通过后，才能将单个规格拆成 `1 viewport × 1 theme × 1 state` 的派生指令；不得直接整段提交并让 Stitch 重新设计页面。

### P1 桌面任务列表 — `1440×900`

```text
Using the global Motrix fnOS context and DESIGN.md, design the desktop Downloading screen at 1440×900 in the default dark theme.

Keep the current structure: a compact fixed left navigation with Downloading active, Completed, Stopped, Trash, and Extensions; auxiliary Settings, Help, About, and Diagnostics entries; a compact top toolbar with Add task, Refresh, Pause visible, Resume visible, and Delete visible; and a scrollable main area of dense horizontal task cards.

Show the shared active, paused, error, magnet-resolving, and file-confirmation task data. Each task must prioritize name and textual status, then progress, downloaded/total size, speed, ETA, category, and contextual actions. Error text appears inline near its task. Actions must match actual state: pause active/pending, resume paused/error, confirm files when required, redownload completed, delete normal tasks, permanent delete only in Trash. Show a visible keyboard focus example, one hovered task action, and one disabled batch action without changing layout dimensions.

Do not turn this into a table with invented columns, add search, count badges, a bottom status bar, a right detail drawer, or seeding categories. Use subtle borders and spacing, not nested cards or persistent heavy shadows.
```

### P2 窄桌面任务列表 — `1024×768`

```text
Adapt the approved desktop task list to 1024×768. Preserve the desktop information architecture and all real actions. Reduce gutters and reorganize secondary metadata without hiding task name, status, progress, speed, ETA, error, or relevant actions. Long file names and URLs must truncate or wrap predictably. Do not switch to the mobile bottom navigation at this width and do not allow horizontal page scrolling. Provide both dark and light theme frames with identical geometry.
```

### P3 桌面空状态

```text
Design the desktop empty state for the current Downloading category at 1440×900. Keep the left navigation and top toolbar visible. Center a compact, calm empty-state composition in the content area with the title 暂无下载任务 and a short specific description. The primary Add task action already exists in the top toolbar, so do not duplicate large marketing CTAs in the desktop content. Show the toolbar Add task action disabled in a separate runtime-exiting variant with a direct service-exiting message. Use a simple existing-style download icon, not a large decorative illustration.
```

### P4 移动任务列表 — `390×844`

```text
Design the Motrix fnOS mobile Downloading screen at 390×844. Use a one-column scrollable task-card list and fixed bottom navigation with safe-area spacing. Preserve the same categories, task data, state logic, and services as desktop.

Show the active, error, and file-confirmation samples. Each card presents name and status first, a readable URL or error, progress, size, speed, ETA, category, and only the valid actions. Every touch action is at least 44×44 pixels. Long Chinese and English text must not push actions outside the viewport. No page-level horizontal scroll. Provide dark and light versions plus one pressed action and one disabled/runtime-exiting state. Do not copy the desktop toolbar into mobile or invent swipe gestures.
```

### P5 新建任务弹窗

```text
Design the existing Add task dialog, first at desktop width and then as a 390×844 mobile modal. Provide four real input modes: URL, Batch URLs, Torrent file, and Magnet link. Keep them in one workflow.

Common fields are authorized save directory, start now or add paused, category, connection count, per-task speed limit, and proxy. URL mode may include file name. Batch mode uses one URL per line and can show partial-success failures. Torrent mode uses a selected .torrent file. Magnet mode validates a magnet:? URI. Use /vol1/downloads as the authorized directory.

Generate separate frames for: clean URL form, invalid URL inline error with disabled submit, authorized directories loading, no authorized directories with a link-like instruction to open fnOS settings, batch partial failures, torrent selected, magnet valid, and submit loading with close disabled. Labels stay above inputs and errors stay below. Footer actions are Cancel and Start download. Do not add username/password, notes, arbitrary path browsing, or future-feature labels.
```

### P6 设置弹窗与主题

```text
Design the current Settings dialog for desktop and mobile. Include only default authorized download directory, maximum concurrent downloads, global download limit, global upload limit, language, JSON-RPC token controls, and the newly approved Dark/Light theme selector. Do not include System theme.

Token controls support hidden/visible value, copy, and secure generation. Directory only uses fnOS-authorized options. Show light-theme selection as an immediate preview while keeping Save and Cancel in a stable footer. Provide frames for loading, unauthorized directory error, empty authorized directory list, save disabled, save loading, and save failure. Do not add notifications, autostart controls, reset defaults, editable Aria2 paths, ports, database locations, or log levels.
```

### P7 任务详情与磁链文件确认

```text
Create two existing dialog states, not a right-side drawer.

First, design a compact task details dialog showing file name, status, progress, size, speed, save directory, file path, GID, URL, created time, updated time, and an optional error reason. It is read-only and closes normally.

Second, design the blocking magnet file-confirmation dialog for Linux 教程合集. Show a selectable file list with checkbox, file name/path, and real sizes. Default all files selected. Provide separate frames for desktop table layout, mobile touch-friendly stacked rows, zero-selection inline error with Start disabled, and confirmation loading with selection and close disabled. Footer actions are Cancel and Start download. Clearly distinguish 正在解析磁链, 等待选择文件, 正在开始下载, and 已开始下载. Do not add connection or log tabs and do not turn this into a persistent task-detail drawer.
```

### P8 About

```text
Design the existing About dialog in dark and light themes. It is a utility information view, not a brand landing page. Show Motrix name and icon, current version 1.6.1, maintainer, target architecture x86_64, repository link, manual FPK update explanation, update status, matching x86/ARM release assets when available, and recent changelog entries.

Provide unchecked, checking, update available, up to date, and unavailable frames. Keep Check for updates as the single prominent action. Do not invent license, build-time, Aria2 engine-version, documentation, or feedback fields unless present in supplied requirements.
```

### P9 Help

```text
Design the existing Help dialog for desktop and mobile using concise information sections for authorized directories, download settings, autostart, Trash, Extensions, and Diagnostics. Clearly label enabled, pending, placeholder, and troubleshooting states without making pending or placeholder items clickable. Keep content dense and readable, with no FAQ accordion, marketing illustration, or invented external links.
```

### P10 Diagnostics 与调试日志

```text
Design the existing Diagnostics dialog and its nested Debug logs dialog. Diagnostics shows app version, backend status, communication status, Aria2 process status, and Aria2 RPC status, followed by the real engine status controls and a Debug logs action.

Debug logs supports level and category filters, timestamps, module, message, repeat count, Refresh, and Clear. Generate frames for status loading, healthy, RPC disconnected, operation loading, empty logs, filtered logs, repeated warning, and clear confirmation. Every status uses text or icon in addition to color. Provide desktop and mobile layouts with long log messages wrapping safely. Do not add charts, resource analytics, terminal emulation, or downloadable diagnostic bundles.
```

## 3. 通用状态补充提示词

```text
For the previously approved screen, generate a state sheet without changing its information architecture. Include default, hover, pressed, focus-visible, disabled, loading, empty, error, and selected states where applicable. Include a reduced-motion annotation. Verify Chinese and English labels, a 56-character file name, a long magnet URI, and /vol1/media/纪录片/2026/超长目录名称 without overlap or horizontal page overflow.
```

## 4. 负面约束复核

每次生成前检查：

- 页面是否出现搜索、右键菜单、底部状态栏、做种/全部任务分类或数量徽标。
- 是否把详情改成右侧抽屉，或加入连接/日志等未实现标签。
- 是否新增通知、开机自启、系统内部路径或其他静态假设置。
- 是否出现 Hero、营销文案、渐变、玻璃、发光、大圆角、嵌套卡片或持续动画。
- 是否缺少浅色主题、移动端、安全区、键盘焦点或错误/禁用状态。

发现任一项时拒绝该稿并使用对应页面提示词重新生成，不在候选稿上继续堆叠修补。

## 5. 未来评估（不得提交 Stitch 首批生成）

搜索、右键菜单、底部全局状态栏、更多任务分类、任务详情抽屉、打开目录、复制链接、任务编辑、通知和系统级 Aria2 设置均为未来候选。只有产品需求文档更新并获得用户明确批准后，才能为其编写页面提示词。

## 6. 迭代记录

| 日期 | Frame | Parent | Viewport / DPR | Theme / State | Prompt | 结论 | 修订说明 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 2026-07-12 | 首轮 48 组产物 | 无稳定父 frame | 元数据不完整 | 多主题/多状态 | v1 | 拒绝进入 Figma | 见 `stitch-output-audit.md`；改用锚点冻结流程 |
