# Motrix fnOS Google Stitch 提示词

> 状态：待用户确认的 Stitch 输入与迭代记录  
> 决策基线：`1A 2B 3B 4B 5A`（2026-07-12）  
> 使用方式：先提交全局上下文，再逐个页面、逐个状态生成；不要一次生成整个应用。

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

| 日期 | 页面 | 版本/截图 | 结论 | 修订说明 |
| --- | --- | --- | --- | --- |
| - | - | - | 待生成 | - |
