# Motrix fnOS Google Stitch 提示词

> 状态：单页 PoC 已完成；当前母版早于 logo 蓝色主题迁移，需按最新 DESIGN.md 复核或修订并获用户确认后再派生其他页面
> 决策基线：`1A 2B 3B 4B 5A`（2026-07-12）  
> 使用方式：先完成第 1 节的单页 PoC。母版通过后，才从已批准 screen 派生第 2 节页面；不得整段提交第 2 节。

## 1. 当前单页 PoC 提示词

本提示词只在项目级 Design System 创建成功后由 Codex 通过 Stitch MCP 提交。不得在没有 Design System 的项目中直接生成，也不得在提示词中重复视觉 token。Stitch 产出只用于视觉和信息架构评审；生成 HTML、CSS、Tailwind class、Material Symbols 或其他图标实现不是项目技术选型。

```text
Create one responsive Web UI screen for Motrix fnOS: the desktop Downloading screen in the default dark theme. Target layout: 1440x900 desktop viewport. Generate only the normal runtime state, not a state sheet.

Use a compact fixed left navigation, a compact top toolbar, and a dense vertical list of horizontal task items. Show active, paused, error, magnet-resolving, and file-confirmation samples. Preserve every real status and only the valid actions. Prioritize task name and textual status, followed by progress, downloaded/total size, speed, ETA, category, error and contextual actions.

Use these samples: active `ubuntu-24.04.2-desktop-amd64.iso`, 3.1 GB / 5.8 GB, 53.4%, 12.7 MB/s, ETA 3m 39s; paused `fnos-backup-2026-07-11.tar.zst`, 847 MB / 2.4 GB, 34.5%; error `documentary-episode-07.mkv`, 1.8 GB / 4.6 GB with `网络连接中断`; magnet resolving `开源纪录片合集`; file confirmation `Linux 教程合集`; category `默认`.

Follow the currently selected custom DESIGN.md exactly. Do not create another design system or substitute a Stitch preset. Do not show hover, focus, pressed, loading or disabled demonstrations in this screen. Do not add Search, System theme, count badges, a bottom speed bar, a right detail drawer, seeding categories, Purge, Export Logs, or any other unapproved feature. Return one desktop screen only.

Express controls with standard component semantics that can be implemented with Vue 3 and Naive UI components such as buttons, tooltips, progress indicators, tags, menus and layouts. Do not treat generated Tailwind classes, Material Symbols, custom fonts, third-party component libraries or static HTML as implementation requirements. Avoid visual controls that would require replacing Naive UI when the same behavior can be represented with its existing primitives and theme overrides.
```

生成后立即停止。不要继续移动端、浅色主题、弹窗或其他状态；Codex 先通过 MCP 读取完整截图和 HTML，并记录评审结论。

局部问题只从当前母版调用 `edit_screens`。MCP 返回的新 screen 属于同一候选的 revision；读取确认后将其设为当前母版，旧 screen 只保留为历史且不再继续编辑。若 MCP 只返回 DOM 操作事件，必须等待 `get_screen` 的截图和 HTML 实际更新后才能认为修订完成。

## 2. 派生页面规格

> 只有第 1 节当前母版通过后，才能逐项使用本节。每次拆成 `1 viewport × 1 theme × 1 state` 的派生指令，并只从已批准的当前母版派生；不得整段提交，也不得从旧 revision 建立分支。

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
Design the existing Add task dialog, first at desktop width and then as a 390×844 mobile modal. Provide three real input modes: Links, Torrent file, and Magnet link. Keep them in one workflow. Links uses one multiline input where each non-empty HTTP/HTTPS line creates an independent task.

Common fields are authorized save directory, start now or add paused, category, connection count, per-task speed limit, and proxy. Links mode does not provide a custom file name and can show partial-success failures. Torrent mode uses a selected .torrent file. Magnet mode validates a magnet:? URI. Use /vol1/downloads as the authorized directory.

Generate separate frames for: clean Links form, invalid link inline error with disabled submit, authorized directories loading, no authorized directories with a link-like instruction to open fnOS settings, multiline partial failures, torrent selected, magnet valid, and submit loading with close disabled. Labels stay above inputs and errors stay below. Footer actions are Cancel and Start download. Do not add username/password, notes, arbitrary path browsing, or future-feature labels.
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
Design the existing About dialog in dark and light themes. It is a utility information view, not a brand landing page. Show Motrix name and icon, current version `<currentVersion>`, maintainer, target architecture x86_64, repository link, manual FPK update explanation, update status, matching x86/ARM release assets when available, and recent changelog entries.

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

## 3. 状态补充提示词

```text
For the previously approved screen, generate a state sheet without changing its information architecture. Include default, hover, pressed, focus-visible, disabled, loading, empty, error, and selected states where applicable. Include a reduced-motion annotation. Verify Chinese and English labels, a 56-character file name, a long magnet URI, and /vol1/media/纪录片/2026/超长目录名称 without overlap or horizontal page overflow.
```
