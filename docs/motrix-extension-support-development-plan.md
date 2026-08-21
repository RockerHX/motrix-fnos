# Motrix Extension 支持调研与开发计划

状态：暂停；ME-07 及真实扩展回归验收无效，等待重新确定扩展基线

版本：v1.1（2026-08-20）

范围：只支持 Motrix Extension 的核心下载转发和任务控制。不把 Aria2 Explorer、AriaNg 或“完整 Aria2 RPC 兼容”纳入本计划。

## 当前暂停说明（2026-08-21）

本计划在 ME-00 冻结时错误地把 `reagin/motrix-extension` 作为目标扩展，随后
ME-01 至 ME-06 实现的是面向该扩展的受控 Aria2 JSON-RPC 兼容层。该实现及其
自动化测试保留在本 feature 分支，不能据此宣称用户当前安装的 Chrome 扩展已
兼容，也不能继续执行原 ME-07 手工验收。

重新核对用户实际安装的扩展和候选项目后确认：

- Chrome Web Store 插件 `djlkbfdlljbachafjmfomhaciglnmkgj` 对应
  `gautamkrishnar/motrix-webextension`，Host 固定为 `127.0.0.1`，仅配置端口
  和密钥，不能连接飞牛 `NAS_IP:17082`。
- `AnInsomniacy/motrix-next-extension` 当前有 Chrome Web Store 版本
  `ofeajdebdjajhkmcmamagokecnbephhl`，但同样固定访问 `127.0.0.1`；它使用
  Motrix Next REST API（`/ping`、`/stat`、`/add`、`/pause-all`、
  `/resume-all`），不是本项目的 JSON-RPC。其作者在
  [Issue #20](https://github.com/AnInsomniacy/motrix-next-extension/issues/20)
  明确说明 JSON-RPC 已从 Motrix Next 和扩展移除。
- 因此不能把 AnInsomniacy 扩展直接替换进当前 JSON-RPC 验收，也不能只修改
  服务端 URL 就完成远端适配。

本次停止原因是兼容目标不成立，而不是已发现当前 JSON-RPC 代码必然错误。后续
必须先重新确定产品路线：要么维护一个支持远端 URL 的 JSON-RPC 扩展 fork，
继续复用本分支服务端；要么另立 Motrix Next REST 兼容项目，同时修改扩展和
服务端。路线确定前，ME-07、真实扩展回归和发布兼容声明均保持暂停。

## 1. 目标和结论

本项目要提供一个受控的 Aria2 JSON-RPC 兼容层，使 Motrix Extension 可以完成以下闭环：

```text
Chrome 下载/右键/磁力链接
        ↓
Motrix Extension
        ↓ HTTP JSON-RPC + LAN Token
Motrix fnOS server /jsonrpc
        ↓ TaskService + SQLite + 生命周期协调器
Aria2 Next sidecar
```

结论如下：

- Motrix Extension 的 RPC client 定义并在功能路径中使用 13 个 `aria2.*` 方法；其中 5 个是连接、统计和列表查询，1 个是创建任务，7 个是任务控制/清理。当前弹窗正常流程直接使用其中 12 个；`aria2.pauseAll` 只在“暂停全部”消息没有携带 GID 列表时作为兼容 fallback 调用，仍属于本轮兼容目标。
- ME-06 完成后，当前服务已通过受控外部白名单实现本计划的 13 个 Motrix Extension 目标方法，并额外保留 `aria2.getGlobalOption` 与 `system.multicall` 兼容入口；所有方法均经过 Token、参数、TaskService 和生命周期边界约束。Issue #11 中 `getVersion` 成功只能证明早期链路和 Token 基本可达；`aria2.getSessionInfo`、`aria2.tellStatus`、`aria2.changeGlobalOption` 等非扩展目标方法仍会返回 `-32601 Method not found`。
- `aria2.getSessionInfo`、`aria2.tellStatus`、`aria2.changeGlobalOption` 等不是 Motrix Extension 当前源码的必需方法，不在本轮外部白名单中。`tellStatus` 可以继续作为 server 内部 RPC 使用。
- 新增方法不能把请求直接透传给 sidecar。所有创建、暂停、恢复、删除、清理都必须经过现有 `TaskService`、任务操作记录、SQLite、目录授权检查和 Aria2 生命周期协调器。
- 内部 Aria2 RPC helper 已能查询或控制部分任务，但不等于对应外部方法已实现；新增 JSON-RPC handler 只能调用统一领域 service，不能在 handler 与 service 中各自向 sidecar 发一次同类 RPC。
- 本轮以 HTTP/HTTPS 下载和磁力链接为支持目标。扩展虽然提供 `ed2k`、`thunder` 开关，但本项目当前任务校验和 Aria2 Next 运行边界不支持这两个协议；它们必须作为明确的非目标，后续另立协议适配事项。

“支持 Motrix Extension”的完成定义是：扩展能连接、显示统计和三类任务列表，能通过扩展创建 HTTP/HTTPS/磁力任务，并能完成单任务/批量暂停恢复、移除和清理；不承诺 AriaNg 的设置管理、Peer 页面或其他 Aria2 客户端功能。

## 2. 调研依据和可复现快照

调研日期：2026-08-20。

### 2.1 Motrix Extension 源码

本计划以仓库 `reagin/motrix-extension` 在 commit `c9e5f5c0c974ddbc03763258d6558d21fa55fb45` 的源码为基线。该 commit 的提交时间为 2026-07-08。

ME-00 于 2026-08-20 重新拉取并冻结以下可复现测试配置：

- 上游默认分支 `main` 仍指向 `c9e5f5c0c974ddbc03763258d6558d21fa55fb45`，方法调用矩阵与本文件一致。
- 最新正式 release 为 [`v2026.07.08.18091`](https://github.com/reagin/motrix-extension/releases/tag/v2026.07.08.18091)，同样指向该 commit。验收使用其 `motrix-extension-2026.07.08.18091-chrome-mv3.zip`，SHA-256 为 `66b9d06a4ab74714baebbfbe002760b3d4f1c72ef6a15038d4328b218998433d`。
- 本地待测 Motrix server 为 `v1.9.3`，仓库提交 `16d1edadf35a003a5804a2f9c57e32fcd32bfdb7`。
- Aria2 Next 固定为 `v2.5.5`；x86_64 sidecar SHA-256 为 `b6f2cdadcd34ba16dd7fcb29de4b84c36f893f9b223a9a05157d1892687a45a0`，aarch64 sidecar SHA-256 为 `fd4b07aeb50fb02a9d19dd55e3ff5cea99e5a6263db1cc6a554c216dc49fa987`。

后续 ME-01 至 ME-07 的协议 fixture、自动化测试和手工验收均以该扩展 ZIP、Motrix server 与 Aria2 Next 版本组合为准。上游默认分支或 release 发生变化时，必须重新执行本节核验后才可更新矩阵。

关键文件：

- [`src/library/rpc/aria2-client.ts`](https://github.com/reagin/motrix-extension/blob/c9e5f5c0c974ddbc03763258d6558d21fa55fb45/src/library/rpc/aria2-client.ts)：RPC 方法、参数、响应 Zod schema、`addUri` 选项和 Token 拼接。
- [`src/library/rpc/types.ts`](https://github.com/reagin/motrix-extension/blob/c9e5f5c0c974ddbc03763258d6558d21fa55fb45/src/library/rpc/types.ts)：任务和统计字段类型。
- [`src/features/background/runtime-state/build-runtime-state.ts`](https://github.com/reagin/motrix-extension/blob/c9e5f5c0c974ddbc03763258d6558d21fa55fb45/src/features/background/runtime-state/build-runtime-state.ts)：弹窗连接成功后的并行调用顺序。
- [`src/features/background/messaging/task-actions.ts`](https://github.com/reagin/motrix-extension/blob/c9e5f5c0c974ddbc03763258d6558d21fa55fb45/src/features/background/messaging/task-actions.ts)：单任务、批量暂停和清理语义。
- [`src/features/background/messaging/handle-message.ts`](https://github.com/reagin/motrix-extension/blob/c9e5f5c0c974ddbc03763258d6558d21fa55fb45/src/features/background/messaging/handle-message.ts)：何时调用 `pauseAll`、`unpauseAll` 和 `purgeDownloadResult`。
- [`src/features/background/downloads/handle-download-created.ts`](https://github.com/reagin/motrix-extension/blob/c9e5f5c0c974ddbc03763258d6558d21fa55fb45/src/features/background/downloads/handle-download-created.ts)：浏览器下载如何收集 URL、文件名、Cookie、Referer、User-Agent 和请求头并提交 `addUri`。
- [`src/features/background/protocol/route-url.ts`](https://github.com/reagin/motrix-extension/blob/c9e5f5c0c974ddbc03763258d6558d21fa55fb45/src/features/background/protocol/route-url.ts)：右键和协议链接的提交方式。

Issue 原文：[`RockerHX/motrix-fnos#11`](https://github.com/RockerHX/motrix-fnos/issues/11)。Issue 中用户使用 fnOS 1.8.4，并报告 `getVersion` 成功而 `getGlobalStat`、`tellActive`、`tellWaiting`、`tellStopped`、`getGlobalOption`、`getSessionInfo` 失败。后四个方法中的 `getGlobalOption` 是本项目已有入口，`getSessionInfo` 属于 AriaNg 需求，不属于 Motrix Extension 当前调用矩阵。

### 2.2 Aria2 官方协议

方法签名、返回字段和错误语义以 [Aria2 JSON-RPC manual](https://aria2.github.io/manual/en/html/aria2c.html#methods) 为协议参考。本项目只实现下面的受控子集；“标准签名”不表示要暴露完整 Aria2 能力。

## 3. Motrix Extension 调用矩阵

扩展的 `call()` 会在有 secret 时把 `token:<secret>` 插到参数数组最前面；没有 secret 时不插入 Token。它使用 HTTP `POST`、`Content-Type: application/json`、JSON-RPC 2.0 和随机 UUID 请求 ID。当前扩展不使用 WebSocket，也不发送 JSON-RPC batch。

### 3.1 当前兼容目标的 13 个方法

| ID | 方法 | 扩展调用 | 结果约定 | 优先级 |
|---|---|---|---|---|
| MEX-01 | `aria2.getVersion` | `getVersion()`，连接探测 | `{version: string, enabledFeatures: string[]}` | P0，已存在 |
| MEX-02 | `aria2.getGlobalStat` | 每次弹窗刷新调用 | `downloadSpeed`、`uploadSpeed`、`numActive`、`numWaiting`、`numStopped`、`numStoppedTotal`，全部为字符串 | P0 |
| MEX-03 | `aria2.tellActive` | `tellActive(TASK_KEYS)` | 任务数组 | P0 |
| MEX-04 | `aria2.tellWaiting` | `tellWaiting(0, 20, TASK_KEYS)` | 任务数组，包含等待和暂停任务 | P0 |
| MEX-05 | `aria2.tellStopped` | `tellStopped(0, 20, TASK_KEYS)` | 任务数组，包含完成和错误任务 | P0 |
| MEX-06 | `aria2.addUri` | `addUri([url], options)` | 新 GID 字符串 | P0，已有但需验收 |
| MEX-07 | `aria2.pause` | 单任务暂停 | GID 字符串 | P1 |
| MEX-08 | `aria2.unpause` | 单任务继续 | GID 字符串 | P1 |
| MEX-09 | `aria2.remove` | 活跃/等待任务移除 | GID 字符串 | P1 |
| MEX-10 | `aria2.removeDownloadResult` | 完成/错误/已移除任务的单项清除 | 标准 Aria2 为 `"OK"`；扩展不依赖具体内容 | P1 |
| MEX-11 | `aria2.pauseAll` | 没有传 GID 时的全部暂停 fallback；正常路径逐项调用 `pause` | `"OK"` | P1，兼容 fallback |
| MEX-12 | `aria2.unpauseAll` | 全部继续 | `"OK"` | P1 |
| MEX-13 | `aria2.purgeDownloadResult` | 清空 stopped lane | `"OK"` | P1 |

截至 ME-06 完成，13 个目标的对外实现状态为：

- 13 个目标方法均已进入受控外部白名单；`aria2.getVersion`、`aria2.addUri` 的既有入口保持兼容，ME-03 已按扩展真实 payload 完成验收。
- ME-01 阶段建立了其余 11 个目标方法的兼容层骨架，完成入口级 Token 校验、参数解析、keys 白名单、分页边界、GID 唯一定位、`-32003` 错误映射和响应模型序列化；随后 ME-02 完成只读快照，ME-04 完成单任务控制和清理，ME-05 完成批量控制和清理，ME-06 完成协议、鉴权和回归验证。

上述 11 个兼容方法不需要各自重写一套 Aria2 能力：查询方法读取 Motrix 任务快照；单任务控制复用 `TaskService`；批量方法复用同一 service 的批量领域操作。内部 `aria2.tell*`、`pause`、`unpause`、`remove` 等 sidecar 调用始终只由这些 service 间接触发。

扩展源码中的任务字段请求常量为：

```text
gid, status, totalLength, completedLength, uploadLength,
downloadSpeed, uploadSpeed, connections, numSeeders, seeder,
errorCode, errorMessage, dir, files, bittorrent
```

以 `LAN_TOKEN` 代替实际密钥时，扩展发出的参数形状如下；这些样例应直接转成 server 的请求 fixture：

```json
{"method":"aria2.getVersion","params":["token:LAN_TOKEN"]}
{"method":"aria2.getGlobalStat","params":["token:LAN_TOKEN"]}
{"method":"aria2.tellActive","params":["token:LAN_TOKEN",["gid","status","totalLength","completedLength","uploadLength","downloadSpeed","uploadSpeed","connections","numSeeders","seeder","errorCode","errorMessage","dir","files","bittorrent"]]}
{"method":"aria2.tellWaiting","params":["token:LAN_TOKEN",0,20,["gid","status","totalLength","completedLength","uploadLength","downloadSpeed","uploadSpeed","connections","numSeeders","seeder","errorCode","errorMessage","dir","files","bittorrent"]]}
{"method":"aria2.tellStopped","params":["token:LAN_TOKEN",0,20,["gid","status","totalLength","completedLength","uploadLength","downloadSpeed","uploadSpeed","connections","numSeeders","seeder","errorCode","errorMessage","dir","files","bittorrent"]]}
{"method":"aria2.addUri","params":["token:LAN_TOKEN",["https://example.com/file.zip"],{"header":["Cookie: session=...","Referer: https://example.com/"],"referer":"https://example.com/","user-agent":"Mozilla/5.0","out":"file.zip","dir":"/vol1/downloads"}]}
{"method":"aria2.pause","params":["token:LAN_TOKEN","GID"]}
{"method":"aria2.unpause","params":["token:LAN_TOKEN","GID"]}
{"method":"aria2.remove","params":["token:LAN_TOKEN","GID"]}
{"method":"aria2.removeDownloadResult","params":["token:LAN_TOKEN","GID"]}
{"method":"aria2.pauseAll","params":["token:LAN_TOKEN"]}
{"method":"aria2.unpauseAll","params":["token:LAN_TOKEN"]}
{"method":"aria2.purgeDownloadResult","params":["token:LAN_TOKEN"]}
```

没有配置 secret 时扩展会省略每个 `token:` 参数；正式验收必须使用 Token，因为 Motrix 的 LAN listener 不应以匿名方式执行写操作。

### 3.2 已存在但不是本轮新增目标的方法

| 方法 | 处理方式 |
|---|---|
| `aria2.getGlobalOption` | 保持现有受控实现；扩展当前不调用，Issue 中的 AriaNg 调用不能作为本轮 Motrix Extension 验收条件。 |
| `system.multicall` | 保持现有实现和逐子调用 Token 校验；扩展当前不调用，作为回归测试。 |
| 内部 `aria2.tellStatus`、`aria2.changeOption`、`aria2.getOption`、`aria2.saveSession` | 仅供 server 内部服务使用，不能因本计划而加入外部白名单。 |

### 3.3 明确不实现的方法

本轮不实现 `aria2.getSessionInfo`、`aria2.getOption`、`aria2.changeGlobalOption`、`aria2.getFiles`、`aria2.getPeers`、`aria2.changePosition`、`aria2.addTorrent` 等外部方法。它们属于 AriaNg、其他客户端或后续产品需求，不能从“支持 Motrix Extension”推导出支持义务。

## 4. 参数、响应和内部状态契约

### 4.1 通用 JSON-RPC 规则

- 请求和响应使用 JSON-RPC 2.0，响应必须原样回显客户端 `id`。
- 单请求失败返回 `{jsonrpc:"2.0", id, error:{code, message}}`；不得返回 SPA HTML。
- 读取和写入均必须通过对应 listener 的 Token 范围。公网反代使用 `127.0.0.1:17081` 和公网 Token；局域网扩展直连 `NAS_IP:17082/jsonrpc` 和独立 LAN Token。
- 保持 `getVersion` 的现有匿名连通性兼容；新增的统计、列表和控制方法必须校验当前入口 Token。扩展发送 Token，因此不会受到影响。
- `system.multicall` 中每个子调用独立校验 Token；不能只校验外层数组。
- 方法未知返回现有 `-32601`；参数形状或类型错误返回 `-32602`；Token 错误继续使用现有 `-32001/-32002`。
- 建议新增一个不会被扩展误判为认证失败的 GID 不存在错误（例如 `-32003`）。不要使用 Aria2 错误码 `1` 表示未知 GID，因为扩展会把错误码 `1` 当成认证失败。
- 不记录 Token、Aria2 secret、Cookie、完整代理 URL、带凭据的请求头或完整下载 URL query。RPC 错误日志应脱敏。

### 4.2 任务状态映射

外部 GID 永远是 `DownloadTask.gid`，不能暴露内部任务 ID。session 恢复或 stale GID 重建后 GID 可能变化，旧 GID 应返回“任务不存在”，扩展下一轮刷新会拿到新 GID。

| Motrix 状态 | `tell*` 外部状态 | 归属 |
|---|---|---|
| `Active` | `active` | `tellActive` |
| `Pending` 且有有效 GID | `waiting` | `tellWaiting` |
| `Paused` 且有有效 GID | `paused` | `tellWaiting` |
| `Complete` | `complete` | `tellStopped` |
| `Error` | `error` | `tellStopped` |
| `Removed` | 默认不再暴露 | 外部清理后从扩展列表消失，记录仍在 Motrix 回收站 |
| `confirmationRequired=true` 且没有最终 GID | 不暴露 | 等待 Web UI 选择文件，不能被扩展当作可控制任务 |

磁力 metadata 阶段如果仍有临时 GID，可以按当前 `Active/Pending` 映射；metadata 完成、进入 Motrix 文件确认阶段后 GID 被清空，应从 RPC 列表消失。这是项目现有磁力任务安全语义，不为扩展伪造一个可恢复的 GID。

### 4.3 任务字段序列化

Motrix 内部很多字段是整数或布尔值，而 Aria2 RPC 要求任务进度字段使用字符串。必须集中实现一个 `DownloadTask -> Aria2CompatTask` 转换器，所有 `tellActive/tellWaiting/tellStopped` 复用它。

| Aria2 字段 | 来源/规则 |
|---|---|
| `gid`、`status` | 任务 GID 和上表状态映射；无 GID 的任务不输出。 |
| `totalLength`、`completedLength`、`downloadSpeed` | `u64` 转十进制字符串。 |
| `uploadLength`、`uploadSpeed`、`connections`、`numSeeders` | 当前模型没有可靠值时返回 `"0"`；不得编造 BT peer 数据。 |
| `seeder` | 当前模型没有可靠做种状态时返回 `"false"`。 |
| `errorCode` | 有错误码时返回数字字符串；无错误时固定返回 `"0"`，不得按请求字段随机省略。 |
| `errorMessage` | 有错误时返回脱敏后的可读文本；无错误可省略。 |
| `dir` | `DownloadTask.save_dir`。只返回任务已持久化且通过授权校验的目录。 |
| `files` | 复用 `DownloadTask.files`；索引保持 Aria2 原生 one-based，`selected` 输出 `"true"/"false"`。 |
| `files[].path` | 优先使用任务文件路径；没有运行时 files 时可用已校验的 `file_path` 或 URL 任务的安全推导路径生成单文件兜底。 |
| `files[].uris` | 扩展不读取，首版可以省略；若返回，只能使用任务来源 URL，不得把 Cookie 或请求头放进 URI。 |
| `bittorrent.info.name` | Torrent/Magnet 使用任务文件名或已解析的 BT 名称；普通 URL 可省略。 |

扩展的 `getTaskName()` 会优先读取 `bittorrent.info.name`，其次取 `files` 中选中文件路径的 basename，最后回退 GID。因此即使某些 Aria2 字段只能返回安全默认值，也必须保证普通 URL 至少有正确的文件名和进度字段。

### 4.4 `keys`、分页和统计

- `tellActive([secret, keys])`、`tellWaiting([secret, offset, num, keys])`、`tellStopped([secret, offset, num, keys])` 必须解析并校验 `keys`。只允许返回本计划列出的字段；未知 key 固定返回 `-32602`，不得静默忽略。
- 指定 `keys` 时响应只包含请求字段；未指定或空数组时返回首版支持的全部字段。
- `offset` 支持 Aria2 的非负和负值语义；`num` 必须为非负整数，并设置服务端上限（建议 1000），防止一次请求拖垮响应。扩展只使用 `0,20`，但负值应至少有单元测试。
- waiting 队列顺序使用 Motrix 任务快照中的稳定顺序（建议创建/队列顺序），stopped 顺序使用停止时间或稳定的 `updated_at` 顺序。不要依赖 SQLite 查询未声明的自然顺序。
- `getGlobalStat` 不启动 Aria2，直接从内存任务快照聚合：活动任务下载速度之和为 `downloadSpeed`，当前模型没有上传速度时为 `"0"`，active/waiting/stopped 数量与三个列表的可见任务一致。`numStoppedTotal` 可与 `numStopped` 相同，因为本项目不暴露 Aria2 的 `max-download-result` 配置；必须在契约中固定这一点。

### 4.5 读取不得唤醒 sidecar

`getGlobalStat`、`tellActive`、`tellWaiting`、`tellStopped` 必须只读内存快照，不调用 `ensure_aria2_ready`，不启动进程，不因为查询写 SQLite。后台任务监控在 Aria2 已经运行时每 500ms 更新快照，足以满足扩展 5 秒轮询；扩展刚连接且引擎停止时应得到速度为 0、空列表或最后已知版本，而不是连接错误。

## 5. 13 个兼容方法的实现要求

### 5.1 `aria2.getVersion`

保留现有实现和响应形状。运行中的 sidecar 可以通过内部受生命周期管理的 RPC 获取版本；sidecar 停止时返回缓存版本，首次无缓存返回 `"unknown"`，不启动 sidecar。`enabledFeatures` 首版继续返回现有稳定值（当前为 `[]`），不要因扩展支持而引入不稳定的产品字段。

### 5.2 `aria2.getGlobalStat`

从任务快照聚合，不直接调用 Aria2。必须返回六个字符串字段，即使值为零也不能省略。统计与 `tell*` 列表使用同一份快照，避免扩展看到数量和列表不一致。

### 5.3 `aria2.tellActive`、`aria2.tellWaiting`、`aria2.tellStopped`

实现一个共享的筛选、分页、排序和字段选择流程。不要为每个方法复制序列化逻辑。`tellStopped` 首版只暴露 `Complete/Error`，不暴露 Motrix 回收站记录；外部 `removeDownloadResult` 和 `purgeDownloadResult` 完成后，扩展下一轮刷新应看到任务消失。

### 5.4 `aria2.addUri`

当前 `server/src/api/jsonrpc/add_uri.rs` 已完成部分适配，实施时以扩展实际 payload 做回归，不重写成直通 Aria2：

- 解析 `[token?, uris, options?, position?]`，首版只保证扩展使用的单 URI、无 position 形式。
- 支持扩展生成的 `header`（包含 Cookie 和允许的请求头）、`referer`、`user-agent`、`out`、`dir`。
- `dir` 必须经过 fnOS 授权目录和现有路径校验；缺省时使用服务端默认授权目录。`out` 只能是文件名，不能成为任意路径。
- `out` 校验固定为：去除首尾空格后非空；不得包含 `/`、`\\`、NUL、`.`、`..` 路径段；UTF-8 字节长度不超过 255。校验失败返回 `-32602`，不能把它交给 `Path::join` 或 Aria2 自行解释。
- 对 options 先分离受 server 管理的 `dir` 和 `out`，其余字段复用 `sanitize_aria2_options` 与任务代理私密覆盖规则。允许白名单固定沿用 `server/src/tasks/options.rs::PASSTHROUGH_OPTIONS`；未知字段保持当前兼容行为——安全忽略并不传给 sidecar，而不是因浏览器扩展携带额外无害字段就拒绝整个任务。必须写测试证明未知字段没有进入最终 Aria2 request。
- `header` 只允许字符串数组，总条数上限 64、单项 UTF-8 字节上限 8192、总字节上限 65536；拒绝 CR/LF/NUL 注入。`referer` 和 `user-agent` 必须是字符串，分别限制为 4096/1024 字节并拒绝 CR/LF/NUL。超过限制返回 `-32602`。
- 创建必须经过 `ensure_aria2_ready`、`TaskService::create_download_task`、SQLite 持久化和任务快照广播；未知 RPC 结果按现有操作记录机制对账。
- HTTP/HTTPS 和 `magnet:?` 进入现有任务流程。`ed2k://`、`thunder://` 当前返回明确的不支持错误，不得伪装成成功 GID。
- Cookie、Referer 和 User-Agent 只在本次任务需要的私密选项中流转，不进入任务公开响应、诊断日志或普通设置。

### 5.5 `aria2.pause`、`aria2.unpause`

通过 GID 查找唯一 Motrix 任务，再调用现有 `TaskService` 控制方法。不要直接调用底层 `aria2.pause/unpause` 后跳过 SQLite。

- `pause`：活动或等待任务暂停，返回 GID；完成/错误任务返回明确冲突，不修改文件。
- `unpause`：暂停任务恢复，返回 GID；需要时由生命周期协调器启动 Aria2，并在恢复前按 SQLite 代理意图对账。
- Motrix 扩展把 `waiting` 也显示为可继续状态。对已经是 waiting/pending 且没有实际需要改变的任务，首版可以幂等返回 GID；不能因为 Aria2 的“cannot be unpaused now”让扩展把一个正常等待任务显示成认证失败。该兼容偏差必须有测试和注释。
- 暂停后的最终进度必须复用现有稳定等待逻辑；持久化失败或 RPC 结果未知时按 TaskOperation 回滚/待对账语义返回错误。

### 5.6 `aria2.remove`

Motrix 没有 Aria2 的“只从引擎结果列表删除但保留应用任务”概念。首版定义为：将任务移入 Motrix 回收站、保留用户文件，并在 sidecar 有效时移除其 GID。实现复用 `TaskService::delete_download_task(config, task_id, false)`。

- 活跃/等待任务：先受控移除 Aria2 GID，再持久化 `Removed` 状态。
- GID 已 stale：沿用现有删除流程，确认 Aria2 已无 GID 后仍可完成本地回收站迁移。
- 不得删除下载文件，不得删除授权目录根，不得永久删除 SQLite 记录。
- 返回成功后，`tellStopped` 不再返回该记录；用户可以从 Motrix Web UI 回收站恢复。

### 5.7 `aria2.removeDownloadResult`

对扩展而言，这是完成/错误/已移除任务行的“移除”按钮。首版也映射为“移入 Motrix 回收站且保留文件”，而不是永久删除任务记录：

- 终态任务且 Aria2 正在运行：由一个统一的 TaskService 兼容清理操作完成引擎 result 清理和 Motrix 回收站迁移。handler 不得先直接调用底层 `aria2.removeDownloadResult`，再调用 `delete_download_task`；现有删除流程本身会先尝试 `aria2.remove`，失败后再以 `aria2.removeDownloadResult` 兜底，前置清理会导致重复调用。任一步结果未知必须保留操作记录并让下一次对账收敛。
- Aria2 已自动停止：不为了清理一个已经不存在的运行时 result 而启动 sidecar，直接安全迁移 Motrix 任务到回收站。
- 已经是回收站记录：幂等返回 `"OK"`，不触碰用户文件。
- 结果字符串固定为 `"OK"`，扩展不会使用 GID 返回值。

如果实现过程中发现现有 `TaskService::delete_download_task(false)` 无法表达上述“sidecar 已停止时跳过引擎操作”，应新增一个领域内的兼容清理 service 方法，并让 `removeDownloadResult` 仅调用它；不得在 JSON-RPC handler 中直接改 SQLite 或重复发底层清理 RPC。

### 5.8 `aria2.pauseAll`、`aria2.unpauseAll`

这是批量领域操作，不应简单 `for` 循环吞掉部分失败：

- 先基于同一快照确定目标 GID，按本计划 6.9 固定的顺序逐项调用现有 TaskService 单任务操作；首版不使用并发。
- 成功全部完成才返回 `"OK"`；部分成功必须返回可诊断错误，并广播最新快照。不得假装整个批次成功。
- `pauseAll` 处理 active/waiting；`unpauseAll` 处理 paused。已完成、错误、回收站和无 GID 任务跳过。
- 需要 sidecar 的批次通过生命周期协调器获取一次受控运行上下文，不能每个任务重复启动/停止。
- 扩展在 active lane 有 GID 时会自行逐个调用 `pause`；`pauseAll` 仍需实现以覆盖无 GID fallback 路径。

### 5.9 `aria2.purgeDownloadResult`

扩展点击 stopped lane 的“全部清除”时只发一个无 GID 请求。首版定义为批量把所有可见 `Complete/Error` 任务移入 Motrix 回收站、保留文件，并由同一批量领域操作按需清理仍存在的 Aria2 result；不删除文件、不永久删除数据库记录。handler 不得先调用底层 `aria2.purgeDownloadResult` 再逐任务删除，否则会与每个任务的统一清理流程重复或造成结果不一致。

- 操作必须可重入；已经 Removed 或没有 GID 的记录跳过。
- 部分失败返回错误并保留未完成 TaskOperation，不能静默丢任务。
- 该语义与标准 Aria2 的“只清内存”不同，属于本项目为保持 SQLite 长期事实和回收站语义做的兼容适配，必须在用户文档和验收记录中明确。

## 6. 安全、监听器和配置要求

### 6.1 连接地址

局域网浏览器扩展的推荐配置：

```text
http://<NAS 局域网 IPv4>:17082/jsonrpc
```

并使用 Motrix 设置中生成的独立 LAN Token。不要让局域网扩展访问管理端口 `17080`；不要把 `17081` 和 `17082` 互相转发后再猜测入口身份。

公网 Lucky 反向代理仍只允许转发到 `127.0.0.1:17081/jsonrpc`，使用公网 Token。`17082` 只接受真实 RFC1918 IPv4 TCP 对端；不依赖 `X-Forwarded-For`、`Host` 或其他可伪造 Header。

Issue #11 的 fnOS 1.8.4 只有旧端口时，必须先确认设备安装的版本是否已经包含 17082 listener；没有时应让扩展配置到正确的 17081 反代地址，不能把不存在的 17082 当成后端服务。

### 6.2 Token 和 CORS

- 保持两个 RPC listener 的 Token 隔离，公网 Token 不能访问 LAN 入口，反之亦然。
- 扩展的 HTTP `POST`、`OPTIONS` 预检和现有 CORS/PNA 响应必须通过测试；不得为了扩展而开放管理路由或任意路径。
- 读取接口也校验 LAN Token（`getVersion` 保留既有匿名兼容例外）；Token 失败响应不能包含配置中的 Token 或 secret。
- 请求体、WebSocket 帧大小限制和请求 ID 规则继续使用现有限制。

### 6.3 路径和敏感数据

- 外部 `dir`、`files[].path` 只能来自已授权且已持久化的任务上下文；不得接受客户端提交的任意路径用于状态响应或删除。
- `addUri` 的 `header` 允许 Cookie/Referer 等短期请求上下文，但必须沿用 Aria2 选项白名单和现有日志脱敏。
- `remove`、`removeDownloadResult`、`purgeDownloadResult` 一律保留用户文件；文件删除仍只能由 Motrix Web UI 的受保护 HTTP API 触发。
- 不引入新的 Token、代理 URL、Cookie 或请求头数据库字段；预计不需要 SQLite schema migration。

### 6.4 强制代码落点和职责边界

以下文件边界是实施约束。实现时可以拆分同职责文件，但不能把业务逻辑重新堆回 `methods.rs`，也不能在 JSON-RPC 层重新写一套任务控制或删除流程。

| 文件 | 必须承担的职责 | 明确禁止 |
|---|---|---|
| `server/src/api/jsonrpc/methods.rs` | 方法名分发、统一 JSON-RPC 响应包装、调用 `compat`/`add_uri` handler | 读取任务、构造 `TaskService`、直接调用 Aria2 RPC、修改 SQLite |
| `server/src/api/jsonrpc/compat/mod.rs`（新增） | 兼容方法 handler 的公开入口和子模块组合 | 自己维护任务状态、直接调用 `aria2_rpc`、复制删除/暂停逻辑 |
| `server/src/api/jsonrpc/compat/params.rs`（新增） | Token 后参数形状、GID、keys、offset/num 的唯一解析实现 | 在每个方法中复制参数下标解析或静默忽略尾部参数 |
| `server/src/api/jsonrpc/compat/model.rs`（新增） | Aria2 兼容 DTO、状态映射、字段选择、分页和响应序列化；保持纯函数 | 访问 SQLite、HTTP、Aria2 或任务内存锁 |
| `server/src/api/jsonrpc/compat/read.rs`（新增） | 四个只读方法：读取一次任务快照/授权快照，调用 `model` 聚合 | 调用 `ensure_aria2_ready`、写 SQLite、广播事件 |
| `server/src/api/jsonrpc/compat/control.rs`（新增） | 单任务和批量 handler 的参数接收、service 调用、错误映射、结果包装 | 直接 `for` 循环调用 `aria2_rpc` 或修改任务数组 |
| `server/src/api/jsonrpc/auth.rs` | 复用现有 `JsonRpcAccess` 选择 Proxy/LAN Token；新增兼容方法统一 Token 校验入口 | 为 Motrix Extension 单独创建第三套 Token 或读取管理 Session |
| `server/src/tasks/service/compat.rs`（新增） | 按 GID 定位任务、单任务兼容操作、批量操作、sidecar 可选清理模式、广播前的领域结果 | 解析 JSON-RPC 参数、生成 JSON-RPC 响应、绕过现有 `control.rs`/`delete.rs` |
| `server/src/tasks/service/control.rs` | 保持现有 `pause_download_task`、`resume_download_task` 的回滚、代理对账和 TaskOperation 语义 | 为扩展复制一份简化版 pause/unpause |
| `server/src/tasks/service/delete.rs` | 抽取共享删除内部流程，支持“必须有 Aria2”和“Aria2 已停止则跳过引擎清理”两种模式 | 为 `removeDownloadResult` 新写一套独立 SQLite 删除流程 |
| `server/src/api/mod.rs` 或同职责共享模块 | 提供唯一的 `TaskService` 构造函数，供管理 API、`addUri` 和 compat handler 复用 | 让每个 handler 手写一份依赖注入构造 |
| `server/src/api/jsonrpc/tests.rs` | 参数、鉴权、分发、响应和只读不唤醒测试 | 依赖真实 Aria2 进程才能运行的单元测试 |
| `server/src/tasks/service/tests.rs` | GID 定位、状态机、TaskOperation、删除模式、批量部分失败测试 | 通过 JSON 字符串间接验证 service 内部状态 |
| `docs/api-contract.md` | 发布后外部方法白名单、参数、错误码、非标准回收站语义 | 宣传为完整 Aria2/AriaNg 兼容 |

共享 `TaskService` 构造函数必须包含现有依赖：`SqliteTaskRepository`、`download_tasks`、`next_task_id`、`app_data_dir`、`debug_logs`、`aria2_rpc`、`aria2_lifecycle`、`download_proxy_update_lock` 和 `RuntimeGuard`。抽取构造函数只消除重复，不改变依赖注入和生命周期所有权。

建议的模块接线固定为：

```text
server/src/api/mod.rs
  ├─ mod task_service;
  └─ pub(crate) use task_service::build_task_service;

server/src/api/jsonrpc/mod.rs
  └─ mod compat;

server/src/tasks/service.rs
  └─ mod compat;
```

`build_task_service(&HttpAppState) -> TaskService<'_>` 只负责依赖注入；`api/tasks.rs`、`api/jsonrpc/add_uri.rs` 和 `api/jsonrpc/compat/control.rs` 都调用它。不要让 `compat` 反向依赖 HTTP route 函数，也不要把 `TaskService` 改成全局单例。

### 6.5 统一请求解析和错误契约

先完成下列共享解析规则，再实现具体方法；所有方法必须使用同一套规则，不能各自“宽松解析”。

1. `params` 只能是 JSON 数组或 `null`。`null` 按空数组处理；对象、字符串、数字直接返回 `-32602`。
2. 除 `getVersion` 外，所有本计划方法都先调用 `ensure_compat_token(state, access, params)`，再调用 `strip_token_param`。Token 只能出现在第一个参数，格式必须是非空 `token:<value>`；缺失、错误、未配置分别沿用 `-32001/-32002`。不要把 Token 校验放到业务分支之后。
3. `getVersion` 保留当前匿名兼容例外和现有参数兼容行为：它可以带 Token，也可以不带 Token；本轮不要为了统一而让它开始拒绝旧客户端的额外参数。其余读取方法也必须校验 Token，不能因为“只读”而匿名开放。
4. GID 参数必须是恰好一个非空字符串；去除首尾空格后长度限制为 1～256 字节。不要强制校验十六进制格式，以兼容现有测试和历史 GID；查找只能使用内存任务快照，不能把 GID 拼入路径或 SQL。
5. `keys` 必须是字符串数组；省略或空数组表示返回本计划支持的全部字段。未知字段、重复字段、非字符串元素返回 `-32602`。支持字段固定为 `gid/status/totalLength/completedLength/uploadLength/downloadSpeed/uploadSpeed/connections/numSeeders/seeder/errorCode/errorMessage/dir/files/bittorrent`。
6. `offset` 必须是 JSON 整数，支持非负和负值；`num` 必须是非负 JSON 整数，服务端上限固定为 `1000`。拒绝浮点数、字符串数字和溢出值。
7. 只允许下列参数形状：

   | 方法 | 去掉 Token 后的参数 |
   |---|---|
   | `getGlobalStat` | 空数组 |
   | `tellActive` | `[]` 或 `[keys]` |
   | `tellWaiting` / `tellStopped` | `[offset, num]` 或 `[offset, num, keys]` |
   | `pause` / `unpause` / `remove` / `removeDownloadResult` | `[gid]` |
   | `pauseAll` / `unpauseAll` / `purgeDownloadResult` | 空数组 |

   多一个或少一个参数都返回 `-32602`，不要静默忽略尾部参数。
8. 在 `server/src/api/jsonrpc/types.rs` 增加 `RpcFault::gid_not_found()`，固定错误码 `-32003`、消息 `任务 GID 不存在或已过期`；增加 `RpcFault::task_conflict()`，固定错误码 `-32005`。未知 GID 不得使用 Aria2 错误码 `1`，因为扩展会把代码 `1` 当作认证失败。非认证错误消息不得包含 `unauthorized`、`token` 或 `secret` 等会触发扩展认证正则的词。
9. 所有失败都仍通过现有 `rpc_error(id, code, message)` 返回，HTTP 状态保持 200；只有 JSON-RPC 入口/资源限制层错误才使用现有 HTTP 错误行为。

兼容错误分类必须集中在一个 `map_compat_error` 中，禁止 handler 用字符串包含判断各自映射：

| 领域错误 | JSON-RPC code | 说明 |
|---|---:|---|
| 参数、状态字段、分页、GID 格式不合法 | `-32602` | 客户端输入错误 |
| Token 错误/未配置 | `-32001/-32002` | 沿用现有行为 |
| GID 不存在、为空或已被 stale GID 重建替换 | `-32003` | 扩展下一次刷新会获得新 GID |
| 任务状态不允许、已有并发操作、应用退出 | `-32005` | 可重试或先刷新列表；消息不得触发认证正则 |
| 生命周期 Starting/Stopping 或请求锁超时 | `-32004` | 沿用 `aria2_busy` |
| RPC 结果未知、SQLite/内存锁/授权快照读取失败 | `-32000` | 保留 TaskOperation 后返回内部错误 |

为避免把现有中文错误字符串长期当成 API 类型，`service/compat.rs` 应定义 `CompatTaskError` enum，并在调用既有 TaskService 后立即分类；不得把底层带 URL、路径、代理或 Aria2 secret 的原始错误直接返回给外部客户端。

标准响应 fixture 固定如下，测试必须精确比较字段类型：

```json
{"jsonrpc":"2.0","id":"stat-1","result":{"downloadSpeed":"0","uploadSpeed":"0","numActive":"0","numWaiting":"0","numStopped":"0","numStoppedTotal":"0"}}
```

```json
{"jsonrpc":"2.0","id":"task-1","result":[{"gid":"gid-1","status":"active","totalLength":"1024","completedLength":"256","uploadLength":"0","downloadSpeed":"128","uploadSpeed":"0","connections":"0","numSeeders":"0","seeder":"false","errorCode":"0","dir":"/vol1/downloads","files":[{"index":"1","path":"/vol1/downloads/file.zip","length":"1024","completedLength":"256","selected":"true"}]}]}
```

```json
{"jsonrpc":"2.0","id":"missing-1","error":{"code":-32003,"message":"任务 GID 不存在或已过期"}}
```

### 6.6 只读兼容快照的具体算法

在 `server/src/api/jsonrpc/compat/model.rs` 实现一个纯数据模型，建议包含 `Aria2CompatTask`、`Aria2CompatSnapshot`、`CompatListKind` 和 `CompatKeys`。它必须接受一次 `Vec<DownloadTask>` 和当前授权目录快照，输出统计和列表；同一请求中不得重复读取任务状态或为每个字段重新查询 SQLite。

任务进入外部快照的顺序和条件固定如下：

1. 使用 `TaskService::list_download_task_snapshot()` 获取可见任务；该方法已经排除 `Removed`。兼容层仍需再次检查 GID，防止无 GID 任务被暴露。
2. `gid` 为空或只包含空白的任务一律不输出，也不计入统计。
3. `confirmation_required=true` 且没有最终 GID 的磁力确认任务一律不输出；如果 metadata 阶段仍有有效临时 GID，按其当前 `Pending/Active` 状态输出。
4. 状态映射固定为：`Active -> active`，`Pending -> waiting`，`Paused -> paused`，`Complete -> complete`，`Error -> error`。不得输出 `removed`。
5. active、waiting、stopped 的目标集合分别为 `active`、`waiting/paused`、`complete/error`。统计数量使用完整集合，不受 `tell*` 的 `offset/num` 页面大小影响。
6. 稳定排序固定为：active/waiting 按 `created_at ASC, id ASC`；stopped 按 `updated_at DESC, id DESC`。禁止依赖 SQLite 未声明的自然顺序或 HashMap 顺序。
7. 分页先排序再切片：`offset >= 0` 从 `min(offset, len)` 开始；`offset < 0` 从 `max(len + offset, 0)` 开始；结束位置为 `min(start + num, len)`。`num=0` 返回空数组但不报错。

字段转换必须集中在同一个 `Aria2CompatTask::select(keys)` 中：

| 外部字段 | 精确转换规则 |
|---|---|
| `gid/status` | 使用上面的 GID 和状态映射 |
| `totalLength/completedLength/downloadSpeed` | `u64.to_string()`；缺省为 `"0"` |
| `uploadLength/uploadSpeed/connections/numSeeders` | 当前模型没有可靠值时固定为 `"0"` |
| `seeder` | 当前模型没有可靠值时固定为 `"false"` |
| `errorCode` | 有值返回原始数字字符串；无值固定 `"0"` |
| `errorMessage` | 仅 Error 任务返回脱敏后的错误文本；非错误任务省略 |
| `dir` | 只有 `save_dir` 位于当前授权根目录内时返回；否则省略。URL 任务要求精确授权目录，BT/磁力任务允许其持久化的任务专属子目录，但必须用 `Path::starts_with` 的组件比较，不能用字符串前缀比较。扩展首版可接受绝对路径；不要调用 `trim.file.convertPath` 或返回展示路径 |
| `files` | 每个 `DownloadTaskFile` 转成 Aria2 对象：`index/length/completedLength/selected` 全部为字符串，`path` 使用已验证路径；`uris` 首版省略 |
| 空 `files` | 普通 URL 生成一个 index=`"1"` 的安全单文件兜底；无法证明路径安全时省略 `files`，不得返回任意数据库原文路径 |
| `bittorrent` | Torrent/Magnet 至少返回 `{info:{name:<file_name>}}`；普通 URL 省略 |

`dir`、`files[].path` 的授权检查必须拒绝 `..`、反斜杠和不在授权根目录组件范围内的路径。授权根目录快照读取失败时，读取方法返回 `-32000`，不能把未验证路径原样发给扩展。扩展只依赖文件名和进度时，路径被撤销的任务仍可返回安全的 GID/状态/数值字段，但必须省略路径字段。不要因为某一条路径无法验证就丢弃整条任务；只省略不安全字段。

路径检查必须复用现有安全原则，而不是只做字符串前缀比较：

1. 拒绝 NUL、反斜杠、`.`/`..` 段和空路径；所有比较使用 `Path` 组件。
2. 授权根先 `canonicalize()`；任务目录或文件存在时也 `canonicalize()` 后再比较，阻止符号链接逃逸。
3. 目标尚不存在时，从目标向上找到最近的现有父目录并 `canonicalize()`，再把剩余的纯 lexical 子路径追加比较；找不到现有父目录则省略该路径字段。
4. URL 任务的 `dir` 必须等于某个授权根；BT/磁力任务的 `dir` 可以是任务专属子目录，但必须同时满足 `owned_task_dir` 非空、目标位于授权根内且不是符号链接目录。
5. 该 helper 只返回 `Option<安全路径>`，不能返回错误后让调用方回退原始数据库路径；授权快照读取错误才升级为整个读取请求的 `-32000`。

`getGlobalStat` 必须在同一份完整快照上计算六个字段：`downloadSpeed` 为 active 任务速度之和，`uploadSpeed` 固定 `"0"`，`numActive/numWaiting/numStopped/numStoppedTotal` 分别为完整集合数量并转成字符串。读取方法不得调用 `ensure_aria2_ready`、不得写 SQLite、不得广播无变化事件；任务内存锁失败才返回内部错误。

### 6.7 单任务兼容操作的固定流程

在 `server/src/tasks/service/compat.rs` 增加按 GID 定位和操作方法。handler 不得先把 GID 转成 task ID 后自己调用多个 service；GID 定位、状态检查和幂等判断必须在 service 内完成。按 GID 定位必须读取 `TaskMemoryState::list()` 的完整任务集合（包含 `Removed`），不能调用会过滤回收站的 `list_download_task_snapshot()`；只有只读 `tell*` 才使用可见快照。

建议的领域方法（名称可按现有风格调整，但职责必须等价）：

```text
find_task_by_aria2_gid(gid) -> Result<Option<DownloadTask>, CompatTaskError>
pause_by_aria2_gid(config, gid) -> Result<DownloadTask, CompatTaskError>
unpause_by_aria2_gid(config, gid) -> Result<DownloadTask, CompatTaskError>
remove_by_aria2_gid(config, gid) -> Result<DownloadTask, CompatTaskError>
remove_download_result_by_aria2_gid(optional_config, gid) -> Result<CompatRemoveResult, CompatTaskError>
```

每个方法遵守以下状态表：

| 方法 | 可操作状态 | 幂等状态 | 引擎要求 | 成功结果 |
|---|---|---|---|---|
| `pause` | `Active/Pending` | `Paused` 直接返回当前 GID | 真正改变状态时必须已有受控 Aria2 配置 | GID 字符串 |
| `unpause` | `Paused` | `Pending/Active` 直接返回当前 GID | `Paused` 必须通过生命周期协调器取得配置 | GID 字符串；stale GID 重建后返回新 GID |
| `remove` | `Active/Pending/Paused` | 已 `Removed` 可返回成功但不重复操作 | 活跃任务必须取得受控 Aria2 配置 | 输入 GID 字符串 |
| `removeDownloadResult` | `Complete/Error` | `Removed` 返回 `"OK"` | 终态清理只在 Aria2 已运行且 PID/运行态匹配时传入配置；否则传 `None` 跳过 sidecar | 固定 `"OK"` |

完成以下检查后才能取得 `ensure_aria2_ready`：先按 GID 找到任务；未知 GID 返回 `-32003`；确认已经是幂等状态时直接返回；只有确实需要 sidecar 的操作才启动或连接 Aria2。这样，错误 GID、已删除任务和 stopped lane 清理都不会无谓启动 sidecar。

`pause_by_aria2_gid` 必须调用现有 `pause_download_task`；`unpause_by_aria2_gid` 必须调用现有 `resume_download_task`；`remove_by_aria2_gid` 必须调用现有 `delete_download_task(config, task_id, false)`。这些既有方法负责 TaskOperation、代理对账、回滚、SQLite 和内存状态，compat service 只负责 GID 解析、状态门禁和错误分类。

`remove_by_aria2_gid` 在调用删除 service 前必须拒绝 `Complete/Error` 任务并返回 `task_conflict`，因为扩展对这两种状态会调用 `removeDownloadResult`；不得让 `aria2.remove` 和 `aria2.removeDownloadResult` 两条外部语义在同一个任务上交叉竞争。`Removed` 任务只在请求 GID 仍与记录中的旧 GID 相等时幂等返回输入 GID；如果任务已被恢复并换成新 GID，旧 GID 返回 `gid_not_found`。

`removeDownloadResult` 需要把现有删除流程抽成一个共享私有实现，例如 `delete_download_task_impl(config_mode, task_id, delete_files)`：

- 管理 API 的普通删除使用 `config_mode=Required`，行为保持不变。
- 外部终态清理使用 `config_mode=IfRunning(Option<&Aria2Config>)`。`Some(config)` 时复用现有 `aria2.remove` → `aria2.removeDownloadResult` fallback；`None` 时跳过 sidecar block，记录 `aria2_cleanup_skipped_engine_offline`，继续按同一 TaskOperation 流程迁移到 `Removed`。
- 不能先在 handler 调用 `aria2.removeDownloadResult`，也不能在 compat service 里重新改任务状态；所有持久化、回滚和 metadata 处理仍走 `delete.rs` 的共享实现。

外部 `removeDownloadResult` handler 的构造方式固定为：先读取任务并判断其状态；若是 `Complete/Error` 且 Aria2 已运行，使用当前运行态配置；若 sidecar 已停止，使用 `None`；然后直接构造 `TaskService` 并调用 compat service。不得调用 `TaskMutationContext::prepare()`，因为该 context 会无条件执行 `ensure_aria2_ready`，会违反“终态清理不唤醒 sidecar”的约束。管理 API 的 `DELETE /api/tasks/:id` 保持现有 `TaskMutationContext::prepare()` 行为不变。

### 6.8 方法分发的固定骨架

`methods.rs` 的新增分支只能采用以下结构，具体函数名可调整但调用顺序不得改变：

```rust
match method {
    "aria2.addUri" => add_uri(state, access, params).await.map(Value::String),
    "aria2.getGlobalStat" => compat::read::get_global_stat(state, access, params).await,
    "aria2.tellActive" => compat::read::tell_active(state, access, params).await,
    "aria2.tellWaiting" => compat::read::tell_waiting(state, access, params).await,
    "aria2.tellStopped" => compat::read::tell_stopped(state, access, params).await,
    "aria2.pause" => compat::control::pause(state, access, params).await,
    "aria2.unpause" => compat::control::unpause(state, access, params).await,
    "aria2.remove" => compat::control::remove(state, access, params).await,
    "aria2.removeDownloadResult" => compat::control::remove_download_result(state, access, params).await,
    "aria2.pauseAll" => compat::control::pause_all(state, access, params).await,
    "aria2.unpauseAll" => compat::control::unpause_all(state, access, params).await,
    "aria2.purgeDownloadResult" => compat::control::purge_download_result(state, access, params).await,
    "aria2.getGlobalOption" => get_global_option(state, access, params).await,
    "aria2.getVersion" => get_version(state).await,
    _ => Err(RpcFault::method_not_found(...)),
}
```

每个 compat handler 的固定顺序是：

1. 调用统一 Token 校验；
2. 解析去 Token 参数并拒绝错误形状；
3. 只读方法读取一次任务/授权快照，控制方法调用 `TaskService` compat service；
4. 将领域错误通过唯一 `map_compat_error` 转成 `RpcFault`；
5. 成功时只返回扩展约定的字符串、对象或数组，不泄露内部 task ID；
6. 状态发生变化时广播一次快照，读取方法绝不广播。

`system.multicall` 不增加特殊旁路：每个子调用仍进入上述分发并独立执行 Token 校验；一个子调用失败只写入 multicall fault object，不影响其他子调用。

### 6.9 批量操作的确定性算法

首版固定采用“单次快照、顺序执行、每项独立 TaskOperation、不中途吞错”的算法，不引入并发，避免同一任务锁、Aria2 生命周期和 SQLite 写入交叉造成不可复现状态。

1. `pauseAll`：读取一次可见任务快照，选出 `Active/Pending` 且有 GID 的任务，按 `created_at ASC, id ASC` 顺序逐项调用 `pause_by_aria2_gid`；已 `Paused`、终态、Removed、无 GID 和 confirmation 阶段跳过。每项 service 调用可以复用同一已获取的运行配置，但不能绕过 TaskOperation；如果现有 pause service 只能自己取得配置，应先增加接收运行上下文的内部变体，不能在批量层直调 Aria2。
2. `unpauseAll`：读取一次快照，只选 `Paused` 且有 GID 的任务；目标非空时批次开始前只取得一次 Aria2 运行上下文，逐项调用 `unpause_by_aria2_gid`。目标为空时不得启动 Aria2。stale GID 的重建必须继续使用现有 `resume_download_task` 语义，并将新 GID 写回任务后才算该项成功。
3. `purgeDownloadResult`：读取一次完整 stopped 集合，按 `updated_at ASC, id ASC` 顺序逐项调用 `remove_download_result_by_aria2_gid`；已 Removed 或无 GID 跳过。绝不调用底层 `aria2.purgeDownloadResult`。若 Aria2 当前运行，批次复用一次已确认的配置；若已停止，全部项目跳过 sidecar 清理并直接走统一回收站迁移。
4. 每项成功立即持久化并完成自己的 TaskOperation；某项失败时记录该项错误，继续处理后续项。结果汇总为：目标为空或全部成功返回 `"OK"`；部分或全部失败返回 `-32006`，消息只包含稳定的失败数量（当前格式为“批量操作失败：N 个任务失败”），不伪造成功。跳过项不计入失败。
5. 批次结束后最多广播一次最新任务快照；即使部分失败也必须广播已成功迁移/暂停的任务状态。每个子任务的未知 RPC 结果由已有 TaskOperation 和启动对账机制处理。
6. 批量方法的 JSON-RPC handler 只负责 Token、空参数、调用 service、广播和结果包装；不得在 handler 中 `for` 循环直接调用 `aria2_rpc` 或修改任务数组。

### 6.10 具体实现顺序和每步完成门槛

按以下顺序实施，后一步不得掩盖前一步的失败：

1. **共享依赖和错误基础**：抽取唯一 `TaskService` 构造函数；增加 `gid_not_found/task_conflict`；增加统一 compat Token/参数解析测试。门槛：既有 `getVersion`、`getGlobalOption`、`addUri` 和 multicall 测试全部保持通过。
2. **纯快照模型**：先在无 HTTP/Aria2 的单元测试中实现 `server/src/api/jsonrpc/compat/model.rs` 的状态映射、字段字符串化、keys、授权路径、排序和分页。门槛：所有 fixture 只通过纯函数测试，不需要启动 sidecar。
3. **只读 JSON-RPC**：新增四个读取分发；handler 调用 service/快照，不启动 Aria2。门槛：扩展四个并行读取请求可被 Zod schema 解析，进程状态和 SQLite 写入计数不变。
4. **单任务 service 适配**：增加 GID 定位、状态门禁和四个单任务 handler；先完成 active/waiting/paused，再完成终态清理的 optional-config 删除模式。门槛：每个成功操作只有一条领域流程，删除 fallback 不重复。
5. **批量 service**：按 6.9 的顺序算法实现三个批量方法。门槛：部分成功可观察、后续任务继续处理、重复调用幂等、无目标不启动 sidecar。
6. **协议和端到端回归**：补 `docs/api-contract.md`、JSON-RPC HTTP/WS/LAN 测试和真实扩展验收。门槛：外部白名单精确为本计划 13 项加已有 `getGlobalOption/system.multicall`，没有任意 RPC 透传。

每个阶段的提交必须保持可编译、可测试、可回滚：不要在一个提交中同时新增四类读取、四类单任务控制和三类批量控制。建议提交边界为 `compat-foundation`、`compat-read`、`compat-single-control`、`compat-batch-control`、`compat-e2e-docs`；提交信息可不同，但评审必须能按此边界定位回归。

### 6.11 最小可交付测试清单

实现代码前先把以下测试名称或等价测试加入计划；低级模型应按测试失败逐项修复，而不是一次性写完所有 handler：

- `compat_token_is_required_for_all_methods_except_get_version`
- `compat_rejects_wrong_arity_non_array_and_unknown_keys`
- `compat_maps_all_task_states_and_hides_removed_or_missing_gid`
- `compat_serializes_numeric_fields_as_strings_and_files_one_based`
- `compat_omits_paths_when_save_dir_is_not_currently_authorized`
- `compat_paginates_positive_and_negative_offsets_deterministically`
- `get_global_stat_does_not_start_aria2_or_write_database`
- `pause_by_gid_reuses_task_service_and_is_idempotent_when_paused`
- `unpause_pending_is_idempotent_and_does_not_start_aria2`
- `unpause_stale_gid_reuses_existing_readd_flow`
- `remove_download_result_skips_sidecar_when_engine_is_stopped`
- `remove_download_result_uses_single_remove_fallback_when_engine_is_running`
- `purge_does_not_call_aria2_purge_download_result`
- `batch_failure_is_reported_after_remaining_tasks_are_attempted`
- `batch_empty_target_does_not_start_aria2`
- `multicall_validates_each_child_token_for_new_methods`
- `lan_and_proxy_tokens_cannot_cross_for_new_methods`

每个测试都必须断言至少一项可观察事实：返回 JSON、任务内存状态、SQLite 状态、TaskOperation phase/status、Aria2 mock 请求序列、进程是否启动或运行时事件数量。只断言函数返回 `Ok` 不算完成。

## 7. 分阶段开发任务

### ME-00：冻结调研基线和兼容声明

依赖：无。

工作：

1. 在实施开始前重新拉取 Motrix Extension 默认分支和最新 release，确认 `aria2-client.ts` 的方法调用没有变化；若变化，更新本文件的方法矩阵和验收样例。
2. 固定实际测试使用的扩展 commit/release、Aria2 Next 版本和 Motrix server 版本。
3. 在设置/JSON-RPC 指南中明确 LAN 地址、Token、17081/17082 用途以及 ed2k/thunder 当前不支持。

完成条件：有可复现的扩展构建或 release ZIP，有记录的 commit 和测试配置，不再以 Issue 中的模糊版本号作为唯一依据。

完成记录（2026-08-20）：已重新拉取默认分支并下载、校验最新 release ZIP；基线、Motrix server、Aria2 Next、端口、Token 和非目标协议声明已记录于本文件、`docs/api-contract.md` 与应用内 JSON-RPC 指南。

### ME-01：建立外部兼容模型和方法分发

依赖：ME-00。

建议范围：`server/src/api/jsonrpc/compat/`、`server/src/api/jsonrpc/methods.rs`、`server/src/api/jsonrpc/types.rs`、`server/src/tasks/service/compat.rs`、`server/src/tasks/service/delete.rs`。不要新建 `server/src/tasks/aria2_compat.rs`；字段兼容模型属于 JSON-RPC adapter，不属于 Aria2 sidecar transport。

工作：

1. 将方法解析、Token 处理、参数解析、错误映射和响应序列化拆成可测试的边界；`methods.rs` 只做分发和统一响应包装。
2. 增加 `Aria2CompatTask`、全局统计和字段选择的内部类型/转换器，集中处理字符串数字、默认值、状态映射、文件和 BT 名称。
3. 明确 GID 查询、分页排序、`keys` 白名单和 `-32003` GID 不存在错误。
4. 保证 `system.multicall`、旧 `getVersion`、旧 `getGlobalOption` 的响应和 Token 回归不变。
5. 抽取 `api::tasks::task_service`（或同等共享构造函数），让 `add_uri` 和 compat service 使用同一依赖注入路径；只读方法不构造需要 `ensure_aria2_ready` 的 mutation context。

完成条件：不接入真实 sidecar 也能通过单元测试验证 13 个方法的参数形状和响应 JSON。

完成记录（2026-08-20）：已抽取共享 `TaskService` 构造入口，新增 `compat` adapter 的参数、Token、错误和响应模型；`addUri` 与兼容入口复用同一服务依赖注入路径。新增参数/模型、GID 唯一定位、JSON-RPC `id` 回显、multicall 子调用鉴权和旧入口回归测试；未启动真实 sidecar，`getGlobalStat`/`tell*` 的快照读取留给 ME-02，控制/清理业务留给 ME-04/ME-05。

### ME-02：实现只读统计和任务列表

依赖：ME-01。

建议文件：`server/src/api/jsonrpc/compat/read.rs`、`server/src/api/jsonrpc/compat/model.rs`、`server/src/api/jsonrpc/compat/params.rs`、`server/src/api/jsonrpc/methods.rs`、`server/src/api/jsonrpc/tests.rs`。

工作：

1. 实现 `getGlobalStat`、`tellActive`、`tellWaiting`、`tellStopped`。
2. 只读内存快照，不启动 Aria2，不写 SQLite，不产生无变化日志。
3. 覆盖任务状态、磁力确认阶段、GID 变化、分页负 offset、keys 筛选和字段默认值。
4. 用固定 fixture 构造 URL、暂停、活动、完成、错误、Removed、BT 和磁力 metadata 任务。

完成条件：扩展连接后的四个并行读取请求均返回 2xx/合法 JSON，Zod schema 可解析，停止 sidecar 时仍能返回稳定的零值/空列表。

完成记录（2026-08-20）：已实现 `getGlobalStat`、`tellActive`、`tellWaiting`、`tellStopped` 的内存快照读取。查询严格匹配当前授权目录，过滤 Removed、无 GID 和磁力文件确认阶段不可见任务；active/waiting 按创建顺序、stopped 按更新时间倒序并以任务 ID 稳定收敛，支持负 offset、分页上限和 keys 字段选择。统计与列表复用同一兼容转换器，数字字段保持 Aria2 字符串格式，错误信息脱敏，读取不启动 sidecar、不写 SQLite、不改变生命周期状态。固定 fixture 已覆盖 URL、暂停、活动、完成、错误、Removed、BT、磁力 metadata、授权过滤和负 offset。

### ME-03：完成 `addUri` 的 Motrix Extension 兼容验收

依赖：ME-01；可以与 ME-02 并行开发，但必须在 ME-05 前完成。

建议文件：`server/src/api/jsonrpc/add_uri.rs`、`server/src/api/jsonrpc/tests.rs`、`server/src/tasks/options.rs`（仅在需要补充长度/字符校验时修改）、`server/src/tasks/service/create.rs`（仅在现有 service 无法保留扩展选项时修改）。

工作：

1. 使用扩展真实 payload 覆盖 `header`、Cookie、Referer、User-Agent、`out`、默认 `dir` 和显式授权 `dir`。
2. 验证 HTTP/HTTPS 任务进入 SQLite、Aria2、内存快照和 SSE 的统一链路。
3. 验证磁力任务的 metadata 暂停、后续文件确认和 GID 变化。
4. 验证未授权目录、空 Token、错误 Token、未知选项、超长请求头和不支持协议的错误。
5. 不把扩展的 `finalUrl` 诊断字段误当成 `addUri` 的第二个 URL；当前扩展只提交原始 URL。

完成条件：使用真实扩展完成一次需要 Cookie/Referer 的 HTTP 下载和一次磁力 metadata 流程，任务记录和文件路径均符合现有安全契约。

完成记录（2026-08-20）：已按扩展真实 `addUri([url], options)` payload 完成 HTTP/HTTPS 与磁力链路验收。请求头、Cookie、Referer、User-Agent、`out`、默认/显式授权目录均进入统一 `TaskService -> Aria2 -> SQLite -> 内存快照/SSE` 链路；未知选项安全忽略，`out`、目录、请求头类型/长度/控制字符均有边界校验，`ed2k://` 与 `thunder://` 明确拒绝。磁力任务使用应用私有 metadata 临时目录并设置 `pause-metadata`/`bt-save-metadata`，返回临时 GID，任务记录可继续进入文件确认流程。新增 HTTP JSON-RPC、multicall、WebSocket 和直接方法回归 fixture；未泄露 Token、Cookie、代理凭据或完整 URL query。

### ME-04：实现单任务控制和回收站适配

依赖：ME-01、ME-02。

建议文件：`server/src/api/jsonrpc/compat/control.rs`、`server/src/tasks/service/compat.rs`、`server/src/tasks/service/delete.rs`、`server/src/tasks/service.rs`、`server/src/api/jsonrpc/types.rs`、`server/src/api/jsonrpc/tests.rs`、`server/src/tasks/service/tests.rs`。

工作：

1. 实现 `pause`、`unpause`、`remove`、`removeDownloadResult`。
2. 将 GID 解析到 TaskService；禁止 handler 直接调用底层 Aria2 RPC 后再调用 TaskService。`removeDownloadResult` 必须通过一个统一的 service 流程，避免与现有 `remove` → `removeDownloadResult` fallback 重复。
3. 为 sidecar 已停止、stale GID、RPC 结果未知、任务并发操作和数据库失败定义回滚/待对账路径。
4. 固定“外部 remove/清理 = 进入 Motrix 回收站、保留用户文件”的语义，并补充恢复验证。

完成条件：扩展单行按钮对 active、waiting、paused、complete、error 五类任务均能正确更新列表；Web UI 回收站能看到外部移除的任务，下载文件仍存在。

完成记录（2026-08-20）：已接入 `pause`、`unpause`、`remove` 和 `removeDownloadResult` 的 GID 到 `TaskService` 分发。暂停/继续复用现有 Aria2 生命周期、代理对账、任务操作记录和回滚流程；waiting/paused 的兼容幂等行为不会误唤醒 sidecar。`remove` 统一进入 Motrix 回收站并保留用户文件；`removeDownloadResult` 通过同一删除 service 执行，终态任务在 sidecar 已停止时使用 optional-config 本地迁移，不启动引擎，运行中沿用单次 remove/fallback 流程；已 Removed 任务幂等返回 `OK`。新增 JSON-RPC 与 TaskService 回归测试，覆盖停止 sidecar、并发门禁、持久化失败回滚、终态幂等和文件保留。批量控制仍留给 ME-05。

### ME-05：实现批量控制、清理和生命周期协同

依赖：ME-04。

建议文件：`server/src/api/jsonrpc/compat/control.rs`、`server/src/tasks/service/compat.rs`、`server/src/tasks/service/control.rs`、`server/src/tasks/service/delete.rs`、`server/src/api/jsonrpc/tests.rs`、`server/src/tasks/service/tests.rs`。

工作：

1. 实现 `pauseAll`、`unpauseAll`、`purgeDownloadResult`，明确目标集合、固定顺序执行、部分失败和幂等行为；首版禁止并发。
2. 批次只获取一次生命周期运行上下文；读取不启动，控制按需启动，终态清理在 sidecar 停止时不得强行启动。若要让既有单任务 service 复用这份上下文，新增内部 helper 必须保持 `ReadyAria2` 的 activity lease 覆盖整个批次；不得只复制配置后释放 lease。
3. 每个子任务复用现有 TaskOperation 记录；`purgeDownloadResult` 通过统一批量 service 编排，不得先直调 sidecar `purgeDownloadResult` 再逐项迁移。批次失败后能在启动对账或重试中收敛。
4. 每次状态变化广播任务快照，扩展 5 秒轮询和 Web UI SSE 不互相覆盖旧状态。

完成条件：扩展 active/waiting/stopped 三个 lane 的全部操作均能完成；批量部分失败不会丢失任务或误删文件。

批量实现的禁止事项：不得使用 `futures::join_all`、`tokio::spawn` 或无界 `Promise.all` 式并发；不得因一个任务失败提前 `return` 而跳过后续任务；不得把失败任务标记为 `Removed`；不得把部分成功包装成 JSON-RPC `result:"OK"`。批次错误响应至少包含 `failedCount` 对应的稳定数量信息（若现有 `RpcFault` 只能返回字符串，则使用“批量操作失败：N 个任务失败”格式），详细逐任务原因只写脱敏日志和 TaskOperation。

完成记录（2026-08-20）：已实现 `pauseAll`、`unpauseAll`、`purgeDownloadResult` 的固定目标快照、稳定顺序和串行执行。批量控制只获取一次 `ReadyAria2` 配置并由 TaskService 复用；终态清理在 sidecar 停止时走本地回收站迁移，不唤醒引擎。单任务冲突、RPC/持久化失败不会中断后续任务，部分失败返回 `-32006` 和稳定失败数量，失败任务保持原状态；空目标和已 Removed 任务幂等成功。新增领域与 JSON-RPC 测试覆盖批次顺序、部分失败、文件保留、sidecar 未启动和快照广播。

### ME-06：协议、鉴权和回归测试

依赖：ME-02～ME-05。

建议文件：`server/src/api/jsonrpc/tests.rs`、`server/src/api/tests.rs`、`docs/api-contract.md`、`docs/motrix-extension-support-development-plan.md` 的验收记录。

工作：

1. HTTP `POST /jsonrpc`、`OPTIONS`、JSON-RPC batch、WebSocket 现有入口各做最小回归；Motrix Extension 的 HTTP 路径必须有端到端覆盖。
2. 覆盖公网 Token/LAN Token 交叉拒绝、缺 Token、错误 Token、空 Token、`system.multicall` 子调用 Token。
3. 覆盖未知方法、非法参数、未知 GID、应用退出、lifecycle stopping、Aria2 自动停止、连接超时和结果未知。
4. 覆盖 `getGlobalStat` 和列表读取不启动进程、不写 SQLite、不泄露 Token/secret/Cookie/代理凭据。
5. 覆盖分页、keys、字段字符串类型、文件索引 one-based、BT 名称、普通 URL 文件名和错误文本脱敏。

完成条件：新增测试全部通过，既有 `server/src/api/jsonrpc/tests.rs`、TaskService、生命周期和打包静态守卫没有回归。

完成记录（2026-08-20）：已补齐 JSON-RPC HTTP `POST` 的单请求、批量请求和解析错误回归，`OPTIONS` CORS 预检、WebSocket 精确入口与消息限制、LAN 真实 RFC1918 对端过滤；公网 Token、LAN Token、缺失/错误/空 Token 和 `system.multicall` 子调用鉴权均有覆盖。新增未知方法、非法参数、未知 GID、任务状态冲突和 Aria2 `Stopping` 可重试错误测试；只读统计/列表在 sidecar 停止和 SQLite 连接关闭时仍从内存快照返回，不启动进程、不写数据库，错误 URL query 与敏感字段保持脱敏。`docs/api-contract.md` 已同步 13 个 Motrix Extension 方法、入口 Token 规则和 `-32003`～`-32006` 错误语义。全量 Rust 测试与快速提交校验通过。

### ME-07：真实扩展手工验收

依赖：ME-06；需要至少一台目标 fnOS 设备或等价 Linux 运行环境。

详细的现场步骤、证据模板和失败判定见
[Motrix Extension 真实扩展手工验收手册](motrix-extension-manual-acceptance.md)。

使用扩展构建/发布 ZIP，按如下矩阵执行：

| 场景 | 操作 | 预期 |
|---|---|---|
| 连接 | 配置 `NAS_IP:17082/jsonrpc` 和 LAN Token，点击 Test connection | 版本可读，无 `Method not found` |
| 空闲连接 | sidecar 停止时打开 popup | 显示在线/可诊断状态，统计为 0，读取不拉起 sidecar |
| HTTP 下载 | 浏览器下载带文件名、Referer、Cookie 的 URL | Chrome 记录被取消，Motrix 任务出现并正常下载 |
| 任务列表 | 刷新 popup，切换 active/waiting/stopped | 三列表字段可解析，速度/进度/文件名正确 |
| 单任务暂停恢复 | active 行 Pause，再 Resume | 状态和 SQLite/SSE 收敛，代理意图不丢 |
| 批量暂停恢复 | 使用 Pause all/Resume all | 全部目标成功；部分失败有提示且可重试 |
| 单任务移除 | active/waiting 行 Remove | 任务进回收站，文件保留，扩展列表消失 |
| stopped 清理 | complete/error 行 Remove，或 Clear all | 回收站记录保留，文件不删除，sidecar 不必要时不启动 |
| 磁力 | 右键/协议点击 magnet，确认文件后开始 | metadata 和最终 BT GID 变化符合项目流程 |
| 重启 | server/sidecar 重启后重新打开 popup | session/SQLite 任务恢复，stale GID 按现有逻辑重建 |
| 错误 | 错 Token、未授权 dir、停止期间操作 | 扩展显示可理解错误，不显示 secret，不产生越权文件操作 |
| 非目标协议 | 打开 ed2k/thunder | 明确不支持或扩展 fallback；不得返回假 GID |

验收时应保存请求/响应摘要和 server 日志，但必须先脱敏 Token、Cookie、URL query、代理凭据和绝对敏感路径。

### ME-08：发布和 Issue 回告

依赖：ME-07。

工作：

1. 更新 JSON-RPC 指南、LAN 配置说明、支持范围和非目标协议说明。
2. 在 `docs/future-development-plan.md` 或对应版本变更记录中登记实际支持的外部方法白名单和已知偏差。
3. 按项目门禁执行 `pnpm run verify:pre-commit`、`pnpm run verify`、`cargo clippy --manifest-path server/Cargo.toml --all-targets -- -D warnings`，再做双架构 FPK 和至少一台 fnOS 实机验证。
4. 在 Issue #11 回复：端口/Token 链路问题与 `Method not found` 的原因已区分，当前版本支持范围、正确的 17081/17082 配置和升级版本写清楚；不要声称完整 AriaNg/Aria2 兼容。

完成条件：发布产物、方法矩阵、测试证据、配置截图/日志摘要和 Issue 回告互相一致。

## 8. 测试设计清单

测试 fixture 约定：所有新增 compat 测试复用 `server/src/api/jsonrpc/tests.rs` 的 `test_state()`、`write_json_rpc_token()`、`write_accessible_paths()` 和现有 `MockAria2Server`/HTTP mock，不另造一套会绕过真实 `HttpAppState` 的 fake。每个任务 fixture 必须显式填写 `id/gid/status/save_dir/file_name/file_path/files/created_at/updated_at/confirmation_required`；禁止依赖 `Default` 或 serde 缺省让关键状态不明确。

所有 Aria2 mock 都必须记录请求顺序和请求体，并在测试结束前断言：

- 读取方法请求序列为空（不会因为外部查询启动或访问 sidecar）；
- pause/unpause/remove 的请求方法、GID、Token 参数位置正确；
- `removeDownloadResult` 运行态只产生一次统一删除流程允许的 fallback 请求，不允许 handler 前置重复请求；
- `purgeDownloadResult` 外部方法不会产生同名 sidecar RPC 请求；
- 未知结果测试同时断言 TaskOperation 为未完成/待对账，而不是任务被静默标记成功。

### 8.1 解析和序列化单元测试

- 每个方法的 token 有/无、参数位置、空 params、错误类型。
- 任意 JSON-RPC `id`（字符串、数字、null）原样回显。
- `keys` 选择只返回所选字段；未知 key 行为固定。
- 数字全部为字符串；文件索引、selected、status 和 BT name 符合扩展 schema。
- waiting/stopped 正负 offset 和 `num` 上限。
- 普通 URL、BT、磁力 metadata、确认等待、Removed、stale GID fixture。

### 8.2 service 集成测试

- 外部 pause/unpause 使用 TaskService，SQLite 和内存快照一致。
- 外部 remove/removeDownloadResult/purge 不删除用户文件，回收站记录可恢复。
- sidecar stopped 时终态清理不启动进程；active 控制按需受协调器管理。
- RPC 结果未知、数据库失败、快照广播失败、并发操作冲突均有可重试/待对账结果。
- 批量控制部分成功不吞错，重复请求幂等或明确冲突。

### 8.3 HTTP/LAN 安全测试

- `17081` 与 `17082` 入口 Token 不能互换。
- LAN listener 只接受 RFC1918 IPv4 真实对端；代理 Header 不能扩大权限。
- CORS/PNA 预检允许扩展 fetch，但管理路由和未知路径仍为 404。
- 1 MiB HTTP body、WebSocket frame/message 限制保持不变。
- 日志和诊断导出不包含 Token、secret、Cookie、代理凭据和完整敏感 query。

## 9. 已知偏差和后续决策点

以下偏差必须在代码注释、API 契约和用户说明中保持一致：

1. 外部 `remove`/`removeDownloadResult`/`purgeDownloadResult` 为了维护 Motrix SQLite 和回收站事实，会保留任务记录并移动到回收站；它们不是底层 Aria2 的纯内存删除。
   这三者的 sidecar 操作与 SQLite 状态迁移必须由同一个 service 统一编排，并通过 TaskOperation 支持结果未知时的对账；handler 不得额外发送重复的 `removeDownloadResult` 或 `purgeDownloadResult`。
2. `getGlobalStat.numStoppedTotal` 不提供 Aria2 原生的跨上限历史计数，首版与可见 stopped 数相同。
3. upload、connections、BT seeder/peer 等当前模型没有可靠来源的字段使用安全默认值，不伪造实时 peer 数据。
4. 读取 API 使用后台内存快照，不为扩展轮询启动 sidecar；极短暂的状态延迟由 500ms monitor 和下一次轮询收敛。
5. Motrix Extension 的 `ed2k`、`thunder` 开关不是本项目本轮支持承诺；若要支持，必须另建协议解码/转换、文件名和安全测试，不得在本计划中偷偷扩展 `addUri` 白名单。

后续若有人提出 Aria2 Explorer 或 AriaNg 支持，应重新建立各自的源码调用矩阵和安全边界，不能把本计划的 13 个方法直接宣传为“完整 Aria2 兼容”。

## 10. 实施前最终检查

- [ ] Motrix Extension 最新源码已重新扫描，方法矩阵与本文件一致。
- [ ] 目标分支已包含并实测 17082 LAN JSON-RPC listener；旧 v1.8.4 端口说明没有混入新版本验收。
- [ ] 外部白名单仍是受控子集，未引入任意 RPC 透传。
- [ ] 已决定并记录 `Removed` 任务是否从外部 `tellStopped` 隐藏（本计划答案：隐藏）。
- [ ] 已决定并记录终态清理是否启动 sidecar（本计划答案：不为终态清理启动）。
- [ ] 已为 Cookie/Referer/User-Agent、路径授权、代理和 Token 准备脱敏 fixture。
- [ ] 已准备真实扩展 HTTP 下载、弹窗刷新、控制、清理和磁力验收环境。
- [ ] 已明确 ed2k/thunder 为非目标，并在验收中验证不会返回假成功 GID。
