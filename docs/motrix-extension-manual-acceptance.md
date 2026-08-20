# Motrix Extension 真实扩展手工验收

> 适用阶段：ME-07。  
> 验收对象：Motrix fnOS 当前待发布构建与 Motrix Extension `v2026.07.08.18091`。  
> 本文是现场验收步骤和证据模板；支持范围、固定版本和接口契约仍以
> [开发计划](motrix-extension-support-development-plan.md) 和
> [API 契约](api-contract.md) 为准。

## 1. 验收目标

确认真实 Chrome/Chromium 中的 Motrix Extension 可以经局域网 JSON-RPC
入口完成连接、HTTP/HTTPS 与磁力下载、三类任务列表和任务控制闭环，同时不
突破目录授权、Token、路径和生命周期边界。

本次只验收 Motrix Extension 调用的 13 个 `aria2.*` 方法，不把 AriaNg、完整
Aria2 RPC、`ed2k://`、`thunder://` 或种子文件上传纳入通过条件。扩展正常的
“全部暂停”路径可能逐个调用 `pause`；只有未携带 GID 列表的 fallback 才调用
`pauseAll`，因此该 fallback 允许用直接 JSON-RPC 请求补充验证。

## 2. 通过标准

以下条件必须同时满足，才能将 ME-07 标记为完成：

1. 第 5 节的 13 个方法均有通过记录，或已明确记录为扩展未触发而按本文
   指定的直接 JSON-RPC 补充验证通过。
2. 第 6 至 9 节的正常流程、重启恢复和安全边界均通过；任何越权下载、路径
   泄露、Token 泄露、任务丢失或无法恢复的状态都判定失败。
3. 使用真实 RFC1918 IPv4 局域网客户端访问 `17082`，未把 LAN Token 配到
   `17081`、管理端口 `17080` 或公网反向代理。
4. 保存第 11 节要求的脱敏证据。发现失败时保留现场，不清空日志或回收站，先
   记录复现步骤和任务 GID。

## 3. 环境和测试数据

验收开始前填写下表。测试目录必须是专用目录，不能混入用户的实际下载文件。

| 项目 | 本次实际值 | 要求 |
| --- | --- | --- |
| Motrix 提交和 FPK 版本 |  | 包含 `8a61cc2` 及之前的 ME-06 修复 |
| NAS 架构 / fnOS 版本 |  | 与 FPK 架构匹配；fnOS `>= 1.2.0401` |
| 飞牛 App 版本 |  | `>= 1.34.0`，用于授权目录和宿主验证 |
| LAN 客户端 |  | Chrome/Chromium 桌面版，真实 IPv4 为 RFC1918 地址 |
| NAS 局域网 IPv4 |  | 例如 `192.168.x.x`；不要填写 FN Connect 域名或 IPv6 |
| 授权根目录 |  | 新建的空目录，例如 `/vol1/downloads/motrix-me07` |
| HTTP/HTTPS fixture |  | 自己控制的合法文件服务，记录 URL、大小和 SHA-256 |
| 磁力 fixture |  | 自己控制或明确合法、可复现的小型测试种子 |
| 扩展 ZIP SHA-256 |  | `66b9d06a4ab74714baebbfbe002760b3d4f1c72ef6a15038d4328b218998433d` |

准备至少以下 fixture。它们既避免依赖不稳定公网文件，又能覆盖请求头和任务
状态转换：

| 名称 | 用途 | 最低要求 |
| --- | --- | --- |
| `small.bin` | 快速完成、stopped 清理 | 可在 1 分钟内完成 |
| `large-a.bin`、`large-b.bin` | active、暂停和批量控制 | 限速后至少保持 2 分钟可下载 |
| `header-check.bin` | Cookie/Referer 透传 | 服务端要求测试 Cookie 和 Referer，否则返回 403 |
| `test.torrent` / 磁力链接 | BT/磁力流 | 合法、文件很小、至少有一个稳定 seeder |

不要把生产 Cookie、私有下载 URL、代理凭据或真实用户文件用于 fixture。若必须
验证带凭据请求，使用仅用于本次验收且可撤销的 Cookie。

## 4. 部署与连接准备

### 4.1 安装和授权

1. 安装与 NAS 架构匹配的 FPK，在 fnOS 桌面中打开 Motrix，完成 Web 管理初始化。
2. 通过飞牛宿主授权第 3 节的专用根目录；在 Motrix 中刷新授权目录并把它设为
   默认下载目录。确认无权目录没有被列为可访问路径。
3. 打开 Motrix 的“设置 -> JSON-RPC”，启用“局域网 JSON-RPC”。首次启用时，
   服务端只显示一次原始 LAN Token，立即保存到本地密码管理器；之后页面只显示
   掩码。不要使用 Web 管理密码、Aria2 secret 或公网 JSON-RPC Token。
4. 记录页面给出的 `http://<NAS IPv4>:17082/jsonrpc`。LAN 入口关闭时必须返回
   `404`；开启后只接受真实 RFC1918 IPv4 对端，代理 Header 不会改变这一限制。
5. 在扩展发布 ZIP 上执行 SHA-256 校验，解压到临时目录后通过 Chrome
   `chrome://extensions` 的“开发者模式 -> 加载已解压的扩展程序”安装。保留扩展
   版本、扩展 ID 和 ZIP 校验结果。
6. 在扩展连接设置中填写步骤 4 的 endpoint 与**原始** LAN Token，不要添加
   `token:` 前缀。扩展会自行按 Aria2 协议添加此前缀。

### 4.2 扩展前的最小连通性检查

从 LAN 客户端执行以下请求。示例中的占位符仅用于说明，替换时避免把 Token 写入
shell history、截图或 issue。

```bash
curl -sS -X POST 'http://<NAS_IPV4>:17082/jsonrpc' \
  -H 'Content-Type: application/json' \
  --data '{"jsonrpc":"2.0","id":"me07-version","method":"aria2.getVersion","params":[]}'
```

预期 HTTP `200`，响应含 `result.version` 和 `result.enabledFeatures`。然后在扩展
中点击 **Test connection**，预期连接成功且没有 `Method not found`。

以下检查必须额外通过：

| 检查 | 操作 | 预期 |
| --- | --- | --- |
| 精确入口 | 对 `http://<NAS_IPV4>:17082/` 发请求 | `404` |
| LAN 开关 | 关闭 LAN JSON-RPC 后访问 `/jsonrpc` | `404`；重新启用后恢复 |
| Token | 将扩展 Token 改为错误值后 Test connection 或刷新列表 | 受保护方法报鉴权错误，不泄露原 Token；恢复正确 Token 后正常 |
| 端口边界 | 从 LAN 客户端只使用 `17082` | `17080` 不作为扩展 RPC endpoint，`17081` 不暴露给 LAN |

## 5. 13 个方法覆盖表

每行都填写“通过/失败/不适用”、时间、任务 GID 和证据编号。所有请求均应为
HTTP `POST /jsonrpc`，并从扩展的开发者工具或脱敏服务日志确认实际方法名。

| 编号 | 方法 | 触发方式 | 通过判定 |
| --- | --- | --- | --- |
| MEX-01 | `aria2.getVersion` | Test connection | 返回版本和 feature 数组 |
| MEX-02 | `aria2.getGlobalStat` | 打开或刷新 popup | 六个统计字段可解析且均为字符串 |
| MEX-03 | `aria2.tellActive` | 打开 active 列表 | active 任务、速度和进度正确 |
| MEX-04 | `aria2.tellWaiting` | 打开 waiting/paused 列表 | 等待和暂停任务正确 |
| MEX-05 | `aria2.tellStopped` | 打开 stopped 列表 | 完成和错误任务正确 |
| MEX-06 | `aria2.addUri` | 浏览器下载或扩展提交链接 | 返回新 GID，Motrix 中仅创建一个任务 |
| MEX-07 | `aria2.pause` | active 行 Pause | 任务转 paused，返回/显示当前 GID |
| MEX-08 | `aria2.unpause` | paused 行 Resume | 任务重新进入 waiting/active，不丢文件 |
| MEX-09 | `aria2.remove` | active 或 waiting 行 Remove | 任务进入 Motrix 回收站，文件保留，扩展列表消失 |
| MEX-10 | `aria2.removeDownloadResult` | stopped 行 Remove | 记录进回收站，文件保留，扩展列表消失 |
| MEX-11 | `aria2.pauseAll` | 扩展 fallback；未触发时用第 7.3 节请求 | 返回 `"OK"`，只暂停当前授权范围的目标 |
| MEX-12 | `aria2.unpauseAll` | 扩展 Resume all | 返回 `"OK"`，全部目标恢复 |
| MEX-13 | `aria2.purgeDownloadResult` | 扩展 Clear all | 返回 `"OK"`，全部 stopped 任务移入回收站，文件保留 |

## 6. 正常下载、列表和单任务控制

### 6.1 空闲读取不启动 sidecar

1. 等待 Aria2 因空闲自动停止；在 Motrix 诊断页确认 sidecar 未运行。
2. 打开扩展 popup，连续刷新两次，再查看诊断页和服务日志。

预期：扩展显示在线或可诊断的连接状态，统计均为 `0`，列表为空；读取没有拉起
sidecar、没有创建任务、没有写入 SQLite。若 `getVersion` 恰好遇到服务停止窗口，
已进入服务的请求只能返回版本或 `-32004`，不得发生死锁、崩溃或错误地启动引擎。

### 6.2 HTTP/HTTPS 下载和请求头

1. 从浏览器访问 fixture 页面，开始下载 `small.bin`。确认 Chrome 原始下载记录被
   扩展取消，而 Motrix 出现一个新任务和新 GID。
2. 下载完成后，在扩展 stopped 列表和 Motrix 任务页核对文件名、大小、完成进度和
   授权目录中的实际文件 SHA-256。
3. 下载 `header-check.bin`。fixture 服务必须在服务端记录“Cookie 与 Referer 均
   已收到”的布尔结果，不记录它们的值。
4. 下载 `large-a.bin`，在扩展 active 行点击 Pause，等待状态成为 paused；再点击
   Resume，确认恢复同一任务且文件继续增长。随后完成下载。

预期：扩展不会显示内部 task ID；`totalLength`、`completedLength`、速度、文件索引
和 `selected` 能被正确解析。错误文本和日志不得包含 URL query、Cookie、Token 或
代理凭据。

### 6.3 三类列表、分页和稳定顺序

1. 同时准备一个 active 任务、一个 paused/waiting 任务、一个 complete 任务和一个
   已知会失败的任务（例如仅使用本地 fixture 的固定 404 URL）。
2. 在扩展中依次打开 active、waiting、stopped 三个列表，连续刷新至少 10 次。
3. 对 stopped 列表滚动或切换后再返回，确认任务仍按最近 `updated_at` 在前的稳定
   顺序显示；active/waiting 保持创建顺序。记录每次显示的 GID 顺序。

预期：complete/error 只在 stopped，paused 只在 waiting，active 只在 active；同一
稳定状态下的多次刷新不改变顺序。所有数值字段都是字符串，普通 URL 的文件路径
和 BT 名称可正常显示时才显示。

### 6.4 移除与文件保留

1. 对 active 或 waiting 任务执行 Remove，确认扩展列表立刻消失。
2. 在 Motrix 回收站确认任务记录存在，检查已下载的部分文件仍在专用授权目录。
3. 对 completed 和 error 任务执行 stopped 行 Remove；在 sidecar 已停止的状态下
   再重复一次 completed 行 Remove。

预期：active/waiting 使用 `remove`，completed/error 使用
`removeDownloadResult`；二者都只移动任务记录到回收站，不删除用户文件。终态
清理不应仅为清理而启动 sidecar；重复清理是幂等的。

## 7. 批量操作和授权范围

### 7.1 全部暂停、恢复和清理

1. 创建并保持 `large-a.bin`、`large-b.bin` 为 active，再创建一个 paused 任务。
2. 从扩展执行 Pause all；确认 active 目标全部进入 paused，原本 paused 的任务
   保持 paused。再执行 Resume all，确认目标继续下载。
3. 完成两个 `small.bin` 副本和一个 error 任务，执行 Clear all，确认 stopped
   任务进入回收站、文件保留、sidecar 已停止时也不会为该操作启动。

预期：批量过程不提前中止；刷新 popup 与 Motrix 页面后状态一致。若现场使用可控
故障环境让某项失败，返回必须是 `-32006`，消息只含稳定失败数量，已成功项目仍
保持成功状态且可重试。不要在生产 NAS 通过杀进程或破坏用户目录来制造该故障；
该分支已有自动化回归覆盖，手工复现应只在隔离 staging 设备进行。

### 7.2 授权撤销后不得跨目录批量操作

这是本轮 P1 修复的必测项。

1. 临时授权两个专用根目录 `A`、`B`，分别创建可持续下载的 URL 任务。确认两者
   都出现在扩展 active 列表。
2. 在 fnOS 中撤销 `A` 的共享目录授权，仅保留 `B`；在 Motrix 中刷新授权快照。
3. 从扩展执行 Pause all，随后刷新列表和 Motrix 任务页。

预期：`B` 中任务被暂停；`A` 中任务不再从扩展暴露，且保持原状态，没有被批量
操作。恢复授权后可由 Motrix 正常管理 `A` 的任务。测试结束前删除或恢复这两个
专用目录的授权状态。

### 7.3 `pauseAll` fallback 的直接补充请求

若扩展版本未发出 `aria2.pauseAll`，在两个 active 测试任务存在时发送下列请求。
使用后必须以 `unpauseAll` 恢复它们。

```json
{"jsonrpc":"2.0","id":"me07-pause-all","method":"aria2.pauseAll","params":["token:<LAN_TOKEN>"]}
```

```json
{"jsonrpc":"2.0","id":"me07-unpause-all","method":"aria2.unpauseAll","params":["token:<LAN_TOKEN>"]}
```

两次均预期 `result: "OK"`。请求可通过第 4.2 节的 `curl` 模板发送；保存响应时将
`<LAN_TOKEN>` 替换为 `<redacted>`。

## 8. 磁力与 BT 任务

1. 从扩展的右键或协议链接入口提交第 3 节的测试 magnet。确认 metadata 阶段的
   任务按项目既有流程进入 Motrix 文件确认页。
2. 在 Motrix 中确认要下载的文件。记录 metadata 临时 GID、最终 BT GID、任务专属
   目录和文件名；GID 变化是允许的，但旧 GID 不能继续被错误地控制。
3. 在扩展中刷新三类列表，确认最终任务和 `{bittorrent:{info:{name}}}` 可解析，
   目录位于授权根目录下的任务专属子目录。
4. 对最终 BT 任务重复第 6.2、6.4 和 7.1 节的暂停、恢复、单项移除和批量操作。

预期：BT/磁力最终任务不会因其任务专属子目录而从扩展列表或批量目标中消失；移除
后文件保留，回收站记录存在。仍在文件确认且没有最终 GID 的磁力任务不得被伪造为
可控制任务。

## 9. 重启、错误和安全边界

### 9.1 重启恢复

1. 保留一个 paused URL 任务、一个 active URL 或 BT 任务及一个 completed 任务。
2. 用 fnOS 应用中心或 Motrix 正常生命周期入口停止并启动应用；不要以宽泛进程名
   终止进程。
3. 服务就绪后重新打开扩展，记录各列表首次显示和两次刷新后的 GID/状态。

预期：SQLite 与 session 任务收敛；仍有效的任务保持可控，完成任务仍在 stopped，
旧 GID 若被重建则返回 `-32003` 而不是错误地控制新任务。扩展不应因一次 stale
GID 而永久显示认证失败。

### 9.2 非目标协议和参数边界

1. 通过扩展打开 `ed2k://` 与 `thunder://` 测试链接。
2. 对 API 发送未授权目录的 `aria2.addUri` 请求，例如 `dir` 为授权根之外的专用
   测试路径。
3. 对 `aria2.pause` 发送 257 字节 GID；可用任意重复 ASCII 字符构造。

预期：非目标协议明确不支持，绝不返回假 GID。未授权 `dir` 与超长 GID 均返回
`-32602`，不创建任务、不创建目录、不启动下载。未知或已经过期的合法形状 GID
返回 `-32003`。

未授权目录请求示例：

```json
{"jsonrpc":"2.0","id":"me07-denied-dir","method":"aria2.addUri","params":["token:<LAN_TOKEN>",["https://fixture.invalid/small.bin"],{"dir":"/not-authorized"}]}
```

### 9.3 符号链接防护（需要 NAS Shell 权限）

此项验证 P1 路径修复，必须只在专用测试目录运行。先在授权根内完成一个可移除的
BT/磁力测试任务，再把它的任务专属目录替换为指向授权根外空目录的符号链接；保留
原目录用于恢复，绝不对用户目录执行此操作。刷新扩展 stopped 列表。

预期：任务的安全 GID、状态和数值字段仍可返回，但 `dir` 与 `files[].path` 被省略；
不得回退输出 SQLite 中的原始路径，不得跟随符号链接访问根外文件。完成观察后移除
测试符号链接并还原原目录，再确认任务/目录恢复正常。没有 Shell 权限时，此项必须
由具备设备维护权限的验收者补测，不能标记为跳过。

### 9.4 生命周期停止竞态

在隔离的 staging 设备上，从 LAN 客户端循环发送匿名 `aria2.getVersion`，同时通过
正常 fnOS 生命周期入口停止应用。记录停止前、停止中和停止后的响应。

预期：已进入服务的请求只得到有效版本响应或 `-32004`；连接断开可记录为传输层
停止现象，但不能出现 panic、长时间挂起、错误启动 sidecar 或返回携带 secret 的
错误。应用重新启动后 Test connection 必须恢复正常。

## 10. 观察与取证

1. 在 Chrome `chrome://extensions` 中打开该扩展的 service worker 开发者工具，或
   使用 Motrix 诊断页，记录 method、HTTP 状态、JSON-RPC code、GID 和时间。不要
   导出完整请求头或原始 Token。
2. 需要日志时优先在 Motrix “诊断”中开启一次性的详细日志、重现问题、立即导出
   诊断包。诊断包按设计会脱敏 Token、Cookie、URL query、代理凭据和敏感路径；
   导出前仍应人工抽查。
3. 有 SSH 时，Rust 服务日志位于
   `$TRIM_PKGVAR/logs/server.log`，生命周期日志位于
   `$TRIM_PKGVAR/logs/lifecycle.log`，Aria2 日志位于
   `$TRIM_PKGVAR/aria2/aria2.log`。不要把这些文件未经脱敏直接上传。
4. 对每一个下载记录 fixture 的 SHA-256、实际路径是否位于授权根，以及移除后文件
   是否仍存在。不能以扩展 UI 单独作为文件操作正确性的证据。

## 11. 验收记录模板

将以下模板复制到 Issue、PR 或私有验收记录。敏感信息统一使用 `<redacted>`。

```text
ME-07 验收日期：
验收人：
Motrix commit / FPK version：
NAS architecture / fnOS / 飞牛 App：
Chrome/Chromium version / Extension ID：
Extension ZIP SHA-256：
LAN endpoint：http://<redacted-ip>:17082/jsonrpc
授权根目录：<redacted-path>
HTTP fixture SHA-256：
BT fixture 标识：

方法覆盖：
MEX-01 [pass|fail] 证据：
MEX-02 [pass|fail] 证据：
MEX-03 [pass|fail] 证据：
MEX-04 [pass|fail] 证据：
MEX-05 [pass|fail] 证据：
MEX-06 [pass|fail] 证据：
MEX-07 [pass|fail] 证据：
MEX-08 [pass|fail] 证据：
MEX-09 [pass|fail] 证据：
MEX-10 [pass|fail] 证据：
MEX-11 [pass|fail] 证据：
MEX-12 [pass|fail] 证据：
MEX-13 [pass|fail] 证据：

正常流程、批量授权边界、BT/磁力、重启、未授权 dir、超长 GID、符号链接、停止竞态：
[pass|fail]；证据和现象：

失败复现步骤：
期望 / 实际：
相关时间、GID、脱敏日志或诊断包：
是否阻塞 ME-08：[yes|no]
```

## 12. 失败处理

任何失败先停止继续执行相同破坏性操作，记录 GID、精确时间和当前授权快照。保留
回收站、文件和诊断证据，避免为了“恢复测试环境”而删除可能用于定位的数据。

下列现象必须阻塞 ME-08：未授权目录被写入；扩展列表或批量操作触及已撤销授权的
任务；BT/磁力最终任务无故消失；路径跟随符号链接逃逸；任何 Token/Cookie/私密 URL
进入截图、响应或可导出的日志；重启后任务丢失或错误控制新 GID。单纯 fixture
网络不可用时，记录后更换同等受控 fixture 重测，不将其归类为产品通过。
