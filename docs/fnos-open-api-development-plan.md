# fnOS 开放 API / SDK 接入开发计划

> 状态：阶段 0 x86 核心验证完成并归档；阶段 1 P0 与阶段 2 P1 开发完成，均待正式 `motrix` 身份实机验收
>
> 目的：评估并规划 Motrix 接入飞牛 fnOS 开放 API 与 `@trimjs/web-app` SDK 的实现路径。
>
> 本文是 `FUTURE-FNOS-API-01` 的详细技术计划。事项状态、优先级和启动门禁仍以 `docs/future-development-plan.md` 为准。

## 1. 结论摘要

飞牛开放 API 不只是为了在应用内选择授权文件夹。它覆盖以下几类能力：

1. 应用共享目录授权：让管理员在应用页面中选择并授权目录。
2. 用户个人目录或文件授权：按 fnOS 用户分别授权目录或文件。
3. 文件权限检查：按当前 fnOS 用户检查读、写、删除权限。
4. 路径转换：把 `/vol1/...` 内部路径转换为面向用户的语义化路径。
5. 页面路由：打开文件、文件管理器、文件详情或外部 URL。
6. 平台配置：读取语言、主题、系统版本和格式配置。
7. 页面交互：设置宿主标题、监听主题/语言变化、关闭页面、设置离开提示。

对当前 Motrix，建议按以下优先级接入：

| 优先级 | 能力 | 当前判断 |
| --- | --- | --- |
| P0 | `pickSharedFile`、`trim.file.getSharedAccessibleFolders` | 直接解决应用内授权目录问题，首批实施 |
| P1 | `trim.file.convertPath` | 改善授权目录和任务路径展示，不改变真实路径 |
| P1 | `openFileManager`、`openFile`、`showFileDetails` | 改善完成任务后的文件操作体验 |
| P1 | `setTitle`、`getPlatformConfig`、主题/语言监听 | 改善 fnOS 宿主一致性，需验证不同 WebView 行为 |
| P2 | `pickUserFile`、`authorizeUserFile`、`trim.file.getUserAccessibleFolders` | 需要可靠的 fnOS 用户身份和统一网关，当前不实施 |
| P2 | `trim.file.checkUserACL` | 依赖用户 UID，当前不实施 |
| P2 | 用户/共享授权删除接口 | 需要任务引用、默认目录和回收站冲突保护，不能直接暴露 |

最重要的架构结论是：前端选择器返回的路径不能直接成为后端事实。授权成功后，Rust server 必须通过 fnOS 官方 Unix Socket API 查询共享授权结果，再更新 Motrix 的授权目录快照；否则当前后端仍可能用旧的 `accessible-paths.json` 拒绝新目录。

## 2. 官方能力与 Motrix 场景映射

### 2.1 应用共享授权（P0）

官方 Scope：`trim.file.sharedAccess`

前端 JS SDK：

- `pickSharedFile(params?)`：打开目录选择器，管理员选择要授权给应用的目录。
- `authorizeSharedFile(path)`：已知目录未授权或授权被移除时，请管理员重新确认；阶段 1 不接入，避免前端向 SDK 提交未经后端确认的路径。

后端 API：

- `trim.file.getSharedAccessibleFolders`：查询当前应用实际获得的共享目录授权。
- `trim.file.delSharedAccessibleFolder`：删除应用共享目录授权。

与 Motrix 的匹配关系：

- 当前 Motrix 使用一组不区分用户的授权目录。
- 下载服务以独立应用用户运行，正需要应用级 ACL。
- 现有 `accessible-paths.json` 和 `/api/storage/accessible-paths` 都是应用级目录模型；快照只来源于官方 API。

权限限制：共享授权由 fnOS 管理员执行。普通用户可能收到 `code: 1` 或 `仅管理员可进行此操作`。Motrix 自己的 Web 管理员身份不能替代 fnOS 管理员身份，最终以宿主返回结果为准。

### 2.2 用户个人授权（P2）

官方 Scope：`trim.file.userAccess`

前端 JS SDK：

- `pickUserFile({ directory: true })`：让当前用户选择并授权一个目录。
- `pickUserFile({ directory: false })`：让当前用户选择并授权文件，返回本次文件路径。
- `authorizeUserFile(path)`：按已知目录或文件路径重新申请授权。

后端 API：

- `trim.file.getUserAccessibleFolders`：按 `uid` 查询用户授权目录。
- `trim.file.delUserAccessibleFolder`：按 `uid` 删除用户授权目录。

当前不接入的原因：

- 该模型要求应用可靠识别当前 fnOS 用户，并将其 `uid` 传给后端 API。
- 当前 Motrix 使用自己的 Web 管理 JWT，端口入口不提供可信的 `X-Trim-*` 用户身份 Header。
- 项目已明确暂缓统一网关迁移，不能把浏览器 Header 或 Motrix JWT 推断成 fnOS 用户身份。
- 如果未来接入，下载任务、默认目录、SSE 和文件操作都必须定义用户隔离边界，不能只新增一个目录选择器。

### 2.3 文件权限检查（P2）

官方 Scope：`trim.file.userAcl`

后端 API：

- `trim.file.checkUserACL`

可检查：

- `readable`
- `writable`
- `deletable`

适用场景：多用户环境下，在返回文件列表、执行写入或删除前检查当前 fnOS 用户权限。

当前不接入的原因与用户个人授权相同：没有可信 `uid`，就无法正确判断“当前用户”对路径的权限。应用级共享授权只授予应用服务用户 ACL，并不代表可以绕过使用用户自身的系统权限。

### 2.4 路径转换（P1）

官方 Scope：`trim.file.path`

后端 API：

- `trim.file.convertPath`

用途：将 `/vol1/1000/downloads` 转换为类似“存储空间1/admin 的文件/downloads”的语义化展示路径。

Motrix 使用方式：

- 后端保留真实绝对路径用于任务、Aria2 和安全校验。
- 前端展示目录列表、任务详情和设置项时使用语义化路径。
- 转换失败时回退原始路径，但不改变任何路径权限判断。

### 2.5 页面路由（P1）

无需额外 Scope，前端 JS SDK 提供：

- `openFile(path)`：用宿主默认方式打开文件。
- `openFileManager(path)`：打开文件管理器并定位路径。
- `showFileDetails(paths, options?)`：打开文件详情页，可进入系统权限调整入口。
- `openURL(url, target?, features?)`：打开外部 URL。

Motrix 使用建议：

- 下载完成任务增加“在文件管理器中打开”入口。
- 单文件完成任务可以调用 `openFile`，但必须确认路径来自后端任务记录且仍在授权范围内。
- 诊断或任务详情可调用 `showFileDetails`，不把它当成应用自己的授权绕过入口。
- SDK 不可用或不在支持的 fnOS 宿主中时，不调用任何宿主授权/设置方法，只提示用户在符合版本要求的 fnOS 宿主中打开 Motrix。

### 2.6 平台配置与页面交互（P1）

平台配置：

- 前端 `getPlatformConfig()`：主题、界面语言、系统版本、日期/时间格式等。
- 后端 `trim.system.getPlatformConfig`：通过 Unix Socket 获取系统语言和系统版本。

页面交互：

- `setTitle(title)`
- `$on('os/theme', callback)`
- `$on('os/language', callback)`
- `setExitPageTips(params?)`
- `close()`

限制：主题和语言事件只在 `sdk.isWeb === true && sdk.isStandaloneWeb === false` 的 Web 宿主中支持；移动 App WebView 和独立浏览器不保证支持。Motrix 必须保留现有语言选择和主题初始化逻辑作为回退。

## 3. 当前项目基线与约束

当前授权链路：

```text
fnOS 官方 API（Unix Socket）
  -> Rust server
  -> accessible-paths.json
  -> Rust storage::load_accessible_paths
  -> GET /api/storage/accessible-paths
  -> Vue 目录选择器
```

相关实现：

- `server/src/storage/mod.rs`
- `server/src/api/storage.rs`
- `packaging/fnos/cmd/common.sh`
- `packaging/fnos/cmd/config_callback`
- `src/services/storage.ts`
- `src/features/settings/stores/settingsStore.ts`
- `src/features/tasks/composables/useTaskSaveDirectory.ts`

必须保持的现有约束：

1. 下载目录只能来自后端确认的已授权目录，前端不得提交任意本地路径。
2. Rust 负责路径校验、默认目录选择和任务保存目录安全判断。
3. 用户下载目录、应用数据目录、任务专属目录和授权目录根必须保持现有边界。
4. 不能因为开放 API 查询失败而把任意路径视为已授权。
5. 正式 FPK 要求 fnOS `1.2.0401+`；不维护低版本人工授权兼容链路。
6. 不得把 `TRIM_API_TOKEN` 写入前端资源、SQLite、普通 API 响应、日志或诊断包。
7. 当前端口入口和统一网关的事实不能被新功能静默改变。

## 4. 官方接入前置条件

### 4.1 FPK 声明

官方要求：

- `config/resource` 声明实际使用的 `api-scope`。
- `manifest` 声明 `micro_app=true`，否则 JS SDK 可能无法初始化。

P0 预计增加：

```json
{
  "api-scope": [
    "trim.file.sharedAccess"
  ]
}
```

只有在批准 P1/P2 对应功能后再增加其他 Scope，不要一次声明全部能力。

官方文档列出的最低版本为：

- fnOS：`1.2.0401`
- 飞牛 App：`1.34.0`

正式 manifest 的 `os_min_version` 固定为 `1.2.0401`。安装器拒绝更低版本，运行时不再维护旧版本人工授权回退。

### 4.2 前端 SDK

依赖：`@trimjs/web-app`，当前公开 npm 版本为 `0.4.2`，具体版本需在正式开发启动时重新确认并锁定。

建议新增一个 `src/services/fnos.ts` 适配层，禁止 Vue 组件直接散落调用 SDK。适配层负责：

- 懒加载或单例化 `TrimApp`。
- 等待 SDK 初始化。
- 判断 `isWeb`、`isStandaloneWeb`。
- 统一处理宿主不可用、版本过低、管理员权限不足和用户取消。
- 为共享授权、文件管理器、文件打开和路径展示提供项目内稳定接口；不封装 `openAppSetting`。

### 4.3 后端 Unix Socket API

官方后端 API 调用方式：

```text
Unix Socket: /var/run/trim_open_gateway_apiscope.socket
HTTP:        POST /api/v1/trimapp
Header:      Authorization: Bearer <TRIM_API_TOKEN>
Request:     { reqId, req, appName, data }
```

后端进程从环境变量 `TRIM_API_TOKEN` 读取 token。项目需要先在目标 fnOS 实机确认：

- FPK 声明 `api-scope` 后 token 是否注入 server 进程。
- `motrix_fnos` 运行用户是否有 Unix Socket 访问权限。
- Socket 路径是否固定，或是否存在版本差异。
- API 返回的路径是否为非根绝对 Unix 路径。
- fnOS 服务不可用、token 缺失、Scope 未注册时的错误状态。

## 5. 目标架构

### 5.1 目录授权事实来源

建议将授权目录刷新分为“查询”和“触发刷新”两类：

- `GET /api/storage/accessible-paths`：继续返回当前已确认快照，不因普通页面读取重复调用外部 API。
- 新增受保护的管理写接口，例如 `POST /api/storage/accessible-paths/refresh`：由 Rust 通过官方 API 查询共享目录，成功后原子写入快照并返回结果。

授权事实优先级：

1. 本次官方 API 查询成功：以返回值为准，包括空数组；空数组不能回退到旧快照。
2. 官方 API 暂不可用但存在旧快照：保留旧快照，并向前端返回“状态未知/需要重试”的诊断信息；不能宣称新授权已生效。
3. 初次安装且没有官方快照：目录为空，只显示在支持的 fnOS 宿主中授权的入口。

需要避免的竞态：

- 授权回调后前端先拿到路径，但后端刷新尚未完成时，不能立即创建下载任务。
- fnOS 在应用运行期间撤销目录后，后端下一次刷新必须收敛到空或新列表。
- 刷新失败不能覆盖最后一次已确认快照，也不能清空现有有效目录。

### 5.2 P0 前端授权流程

宿主内：

```text
用户点击“添加授权目录”
  -> TrimApp.pickSharedFile()
  -> fnOS 管理员选择目录并确认
  -> SDK 返回成功/取消/权限错误
  -> Motrix 请求 POST /api/storage/accessible-paths/refresh
  -> 刷新 GET /api/storage/accessible-paths
  -> 更新设置页、新建任务页和默认目录校验
```

独立浏览器：

```text
用户点击按钮
  -> 检测为 isStandaloneWeb=true 或 SDK 不可用
  -> 不调用 openAppAuth、pickSharedFile 或 openAppSetting
  -> 提示用户在符合版本要求的 fnOS 宿主中打开 Motrix 完成授权
  -> 用户返回后手动刷新目录快照
```

授权操作必须由真实用户点击触发。不能在页面加载、定时器或异步刷新中自动打开授权窗口。

### 5.3 前端界面入口

P0 至少提供：

- 设置页授权目录区域：“添加授权目录”按钮。
- 新建任务保存目录为空时的“添加授权目录”按钮。
- 授权目录刚被撤销或刷新失败时的“重新授权”入口。
- 非 fnOS 宿主或 SDK 不可用时的明确支持环境提示。
- 普通用户调用共享授权时显示“需要 fnOS 管理员权限”，不误报为 Motrix 登录失败。

不要在 P0 中提供自由文本路径输入框，也不要仅根据 SDK 返回的路径修改本地授权列表。

### 5.4 共享授权删除策略

官方删除 API 不应直接绑定到现有目录列表的删除按钮。执行删除前至少检查：

- 是否为当前默认下载目录。
- 是否被任何任务保存目录引用。
- 是否被运行中、暂停中、完成或回收站任务引用。
- 是否存在待清理文件、待恢复 metadata 或未完成操作。

第一版只提供“重新授权”，暂不提供应用内删除授权。后续若要提供删除，必须设计明确的冲突响应和迁移语义。

## 6. 分阶段实施计划

### 阶段 0：实机前置验证

状态：x86 核心实机验证完成并归档；ARM 和飞牛移动 App WebView 延期。独立 `motrixapiprobe` 已完成本地实现、双架构构建和静态验收，并在 fnOS `1.2.0401` x86 虚拟机完成管理员主流程、撤销、普通用户权限边界、应用账户读写和生命周期验证。本探针仅是非生产验证资产，不占用正式 `motrix` 身份；其源码、构建接入和产物已在 2026-08-15 移出仓库，不得直接发布或迁入正式包。

已完成的临时交付（2026-08-13）：

- 独立源码与 FPK 工程已在仓库外归档，仓库只保留计划和验收证据。
- 固定身份 `motrixapiprobe`、端口 `17180`、版本 `0.1.0`、最低 fnOS `1.2.0401`。
- 唯一 Scope `trim.file.sharedAccess`，独立运行用户 `motrix_api_probe`，桌面入口 `allUsers=true`。
- Rust Probe API、Unix Socket client、SDK 页面、独立浏览器回调、应用账户读写检查和安全生命周期脚本。
- x86 与 ARM 两个 FPK、SHA-256 清单及最终 FPK 解包静态验收。
- Rust、前端与构建脚本自动化测试；详细报告和归档校验和只保存在开发者本地的 `docs/verification/`，不进入版本库。

实机报告已确认 x86 环境的 `TRIM_API_TOKEN` 注入、Socket 权限、桌面 iframe 调用、普通用户新增授权拒绝、官方目录查询、应用账户读写和停止重启。独立浏览器不会调用 SDK 授权方法；ARM 和飞牛移动 App WebView 仍待目标环境具备后补测。

任务：

1. 使用独立最小 FPK 验证 `micro_app=true` 和 `api-scope` 生效。
2. 在至少一台目标 fnOS 上验证 `TrimApp` 初始化、`pickSharedFile`、取消和非管理员错误。
3. 验证 `TRIM_API_TOKEN`、Unix Socket 权限和 `trim.file.getSharedAccessibleFolders`。
4. 验证 SDK 在桌面 WebView、飞牛 App WebView、独立浏览器三种宿主的行为。
5. 验证应用入口使用现有端口模式时，SDK 是否仍能识别宿主；若不能，单独记录为统一网关迁移依赖，不直接改 Motrix 主包。
6. 记录 fnOS 版本、飞牛 App 版本、FPK checksum、Socket 权限、请求/响应脱敏样例和错误码。

完成标准：可以证明 P0 的宿主调用和后端查询链路在真实设备上可用，或者明确具体阻塞点并保留旧流程。当前 x86 核心验证已满足；延期项不阻塞阶段 1，但必须在对应环境可用后补录报告。

### 阶段 1：P0 共享授权闭环

状态：开发完成，待正式 `motrix` 身份实机验收。自动化测试、Clippy、统一验证、双架构 FPK 构建与解包校验已通过；FPK 与本地验收报告不进入 Git。正式身份验收通过前不将本阶段标记为发布完成。

任务：

1. 锁定 `@trimjs/web-app` 版本并加入依赖校验。
2. 增加 `config/resource` 的 `api-scope` 和 `manifest` 的 `micro_app=true`。
3. 新增 Rust fnOS API client，封装 Unix Socket、Bearer token、JSON 请求/响应和超时。
4. 新增共享授权 service，支持查询、响应校验、路径规范化和快照原子写入。
5. 新增受保护的授权刷新 HTTP API，保持现有 `GET /api/storage/accessible-paths` 兼容。
6. 新增前端 SDK 适配层；独立浏览器不实现 App Auth 或授权回调页。
7. 在设置页和新建任务空目录状态接入“添加授权目录”。
8. fnOS 宿主中只提供 `pickSharedFile()`；独立浏览器和 SDK 不可用时只提供支持环境说明。
9. 更新中英文文案，区分“授权取消”“需要 fnOS 管理员”“宿主不支持”“刷新失败”。
10. 更新 `docs/api-contract.md`、`docs/fpk-packaging.md` 和 UI 产品需求，记录新增接口、Scope、环境变量和回退语义。

完成标准：管理员可以在支持的 fnOS 宿主中完成共享目录授权，后端在官方 API 确认后使用新目录创建任务；独立浏览器或 SDK 不可用时不误调用宿主方法，并显示明确支持环境说明。

### 阶段 2：P1 路径和宿主体验

状态：开发完成，待正式 `motrix` 身份实机验收（2026-08-15）。阶段 2 已完成语义化路径转换、目录和任务展示、完成任务宿主文件操作、主题/语言初始化与宿主降级适配。自动化验证、Rust 测试、前端类型检查与生产构建、双架构 FPK 构建和解包校验均已通过；FPK 与本地验收报告不进入 Git。最终 x86 fnOS `1.2.0401` 实机验收将同时覆盖阶段 1 和阶段 2，ARM 设备待系统版本满足要求后补测。

固定契约：

- 正式 Scope 精确为 `trim.file.sharedAccess` 与 `trim.file.path`，`os_min_version` 固定为 `1.2.0401`。
- fnOS `1.2.0401` / 飞牛 App `1.34.0` 才启用路径转换、平台配置和文件路由；不满足版本要求的系统不能安装正式包。
- 桌面宿主完整跟随并监听主题；移动 WebView 只初始化主题；独立浏览器默认深色且不调用宿主方法。
- Motrix 保存语言优先。宿主语言只用于没有本地偏好的登录前初始化；登录后由 Motrix 配置接管。
- 本阶段只接入 `setTitle`，不接入 `setExitPageTips` 与 `close`。
- 语义路径只用于展示，不缓存、不持久化、不参与授权或文件系统判断。
- 文件操作每次按任务 ID重新查询后端安全上下文；`showFileDetails` 不传 `{ admin: true }`。

任务：

1. 接入 `trim.file.convertPath`，增加后端批量转换接口或在已有目录响应中附带安全展示字段。
2. 目录选择器和任务详情展示语义化路径，真实路径只用于后端和必要的诊断复制。
3. 完成任务增加“打开文件管理器”；单文件任务按安全条件提供“打开文件”。
4. 文件详情入口调用 `showFileDetails`，不把系统详情页当作应用自己的 ACL 检查。
5. 使用 `getPlatformConfig()` 初始化宿主主题、语言和标题；保留当前 Pinia/i18n 设置作为回退。
6. 在确认支持的 Web 宿主中监听主题和语言变化；移动 WebView 和独立浏览器只使用初始化值。
7. 宿主内使用 `setTitle('Motrix')`；`setExitPageTips` 与 `close` 留待后续明确需求。

完成标准：P1 能力不会改变下载安全边界，宿主不可用时页面仍可正常使用。当前开发完成；正式发布前仍需完成正式 `motrix` 身份的 x86 实机验收，并在 ARM 系统升级后补测。

### 阶段 3：统一网关与用户级能力评估

前置门禁：`FUTURE-GATEWAY-01` 完成独立最小 FPK 实机矩阵，并更新架构、API 契约和打包文档。

任务：

1. 通过可信统一网关身份确认当前 fnOS 用户 UID。
2. 重新评估 `pickUserFile` 和 `trim.file.getUserAccessibleFolders` 的数据模型。
3. 定义用户级任务、默认目录、SSE、回收站和 JSON-RPC 的隔离语义。
4. 接入 `trim.file.checkUserACL`，在展示、写入和删除前检查用户权限。
5. 设计用户授权目录删除的冲突检查、审计和撤销行为。
6. 只有在产品确认需要多用户隔离时，才增加 `trim.file.userAccess` 和 `trim.file.userAcl` Scope。

完成标准：用户级能力不把共享目录和个人目录混为一体，不依赖客户端伪造 Header，并通过真实网关转发链路验收。

## 7. API 与错误处理约定

### 7.1 前端 SDK 错误

至少区分：

- `0`：成功。
- `1000000`：宿主或内部异常。
- `1000001`：登录或认证失败。
- `1000002`：管理员权限或 API Scope 不足。
- `1000030`：参数、路径或当前能力不支持。
- `1000300`：应用未安装或未运行。
- `1000701`：路径不存在。
- `1003103`：应用权限校验失败。
- `1003201`：管理员关闭普通用户授权能力。

### 7.2 后端 API 错误

后端内部保留官方 HTTP 状态和业务码用于脱敏诊断，但 fnOS 上游鉴权失败不得映射为 Motrix HTTP `401`，避免前端误清除 Web 管理 JWT。外部 API 的 Token、Authorization Header、原始响应和完整路径列表不得进入日志。

### 7.3 Motrix HTTP 错误

阶段 1 固定错误码：

- HTTP `503`：`fnos_api_token_missing`、`fnos_api_socket_unavailable`、`fnos_api_timeout`、`fnos_api_transport_error`。
- HTTP `502`：`fnos_api_rejected`、`fnos_api_invalid_response`。
- HTTP `500`：`accessible_paths_persist_failed`。

写接口继续遵循 Motrix 管理 API 鉴权：保护开启时要求有效管理员 JWT，保护关闭时允许匿名管理。普通 GET 不应因为读取授权目录而启动 Aria2。

## 8. 数据与安全要求

1. `TRIM_API_TOKEN` 只在 Rust server 内存中按需读取，不写入 SQLite、前端、日志、SSE、诊断包或错误详情。
2. 前端传回的授权路径只用于触发刷新或 UI 状态，不能绕过官方 API 查询。
3. 官方 API 成功返回空列表时，必须原子替换为空列表，不能保留旧路径继续下载。
4. 官方 API 失败时保留最后一次已确认快照，但前端必须知道状态可能过期。
5. 真实内部路径与语义化展示路径分离，路径转换失败不影响权限校验。
6. 任务创建、恢复、重新下载、文件清理和 JSON-RPC 仍统一使用后端授权目录校验。
7. 不能因为当前 Motrix 管理员已登录就假定其是 fnOS 管理员或拥有其他用户 ACL。
8. 共享授权删除前必须检查默认目录、任务引用、回收站、metadata 和文件清理操作。
9. 授权选择窗口必须由用户手势触发；独立浏览器不得启动 App Auth 或伪造授权回调。
10. 旧版本回退路径不得把应用数据目录作为外部下载目录返回，继续遵守现有路径安全规则。

## 9. 自动化测试计划

### 9.1 Rust

- Unix Socket HTTP client：请求结构、Bearer token、超时、非 JSON 响应、HTTP 错误和官方 `code` 错误。
- 官方响应解析：路径数组、空数组、重复路径、空路径、非法路径和未知字段。
- 快照更新：成功替换、空结果替换、失败保留旧快照、并发刷新和原子写入。
- 环境兼容：Socket/token 不存在时保留最后一次官方 API 快照；正式包不读取 `TRIM_DATA_ACCESSIBLE_PATHS`。
- HTTP API：JWT、错误码、刷新后目录读取和 Aria2 不被唤醒。
- 现有任务/设置/JSON-RPC 测试：确认新快照仍经过既有授权目录校验。

### 9.2 前端

- SDK 宿主检测：桌面宿主直调、移动 WebView 直调、独立浏览器和无宿主回退。
- 授权结果：成功、取消、管理员不足、Scope 不足、版本不支持和窗口被拦截。
- SDK 动态加载失败时主页面继续可用，且不调用任何 App runtime 方法。
- 设置页、新建任务页授权按钮的加载、禁用、错误、刷新和重复点击。
- P1 页面路由调用只接受后端返回的已授权任务路径。

### 9.3 打包与静态守卫

- `manifest` 包含 `micro_app=true`，且版本同步检查不被破坏。
- `config/resource` 只声明批准的 Scope。
- 前端构建产物不包含 `TRIM_API_TOKEN`、Unix Socket 路径或后端 Authorization Header。
- 最终 FPK 不包含 App Auth 回调页或独立浏览器授权入口。
- x86_64/ARM64 FPK 解包、SBOM、checksum 和权限文件检查。

## 10. fnOS 实机验收矩阵

| 场景 | 预期 |
| --- | --- |
| 新版本 fnOS，管理员，宿主内入口 | 选择目录、授权、刷新、创建任务成功 |
| 新版本 fnOS，普通用户 | 共享授权被拒绝，提示需要 fnOS 管理员，应用仍可查看已有目录 |
| 非宿主独立浏览器 | 不调用 App runtime，显示支持环境说明并允许重新读取快照 |
| 飞牛 App WebView | SDK 初始化和授权流程可用；不依赖主题/语言事件 |
| 用户取消选择 | 不修改授权快照，不显示为错误 |
| 目录已被撤销 | 刷新后从列表移除，旧默认目录变为未授权 |
| 官方 API 返回空列表 | 列表变空，不使用旧快照继续创建任务 |
| Socket/token 不可用 | 保留已确认快照，提示刷新失败；不清空已有目录 |
| 应用重启 | 重新查询或安全回退，目录状态与实际授权一致 |
| 应用升级 | 保留任务、设置、旧快照和旧客户端兼容性 |
| 共享授权目录被任务引用 | 删除入口隐藏或返回明确冲突，不破坏任务 |
| 路径转换失败 | 继续使用原始路径展示，下载与安全校验不受影响 |
| 未授权路径写入请求 | 后端拒绝，前端不能通过伪造路径绕过 |

## 11. 发布与回滚策略

1. P0 首个版本只面向 fnOS `1.2.0401+`，不提供旧系统人工授权回退。
2. 不因为增加 `api-scope` 就自动改变已有授权目录或默认下载目录。
3. 旧版本升级到新版本时，先保留 `accessible-paths.json`，再尝试官方 API 同步。
4. 回滚到旧版本时，由发布流程恢复对应 FPK 与数据备份；正式包不承诺旧版读取新版本的官方 API 快照语义。
5. 如果实机证明当前端口入口无法初始化 `TrimApp`，不直接修改 Motrix 主包入口；应先完成独立统一网关实验，再重新评估迁移。
6. 发布说明必须标注支持的 fnOS/飞牛 App 最低版本、管理员限制、独立浏览器回调限制和回退方式。

## 12. 开发启动门禁

满足以下条件后，才能把事项从“待规划”改为“进行中”：

1. 用户确认先实施 P0 共享授权闭环。
2. 阶段 0 最小 FPK 在目标 fnOS 实机完成 SDK、Scope、token、Socket 和授权结果验证。
3. 明确当前端口入口是否支持微应用 SDK；如不支持，确认是否启动独立统一网关实验。
4. 完成 `docs/architecture.md`、`docs/api-contract.md`、`docs/fpk-packaging.md` 和 UI 产品需求的事实更新。
5. 冻结快照优先级、空结果语义、失败回退和刷新接口契约。
6. 冻结 token、路径、管理员身份、JWT、回调 state 和日志脱敏方案。
7. 建立与 P0 风险相称的 Rust、前端、脚本和 FPK 测试清单。

## 13. 官方资料与仓库依据

官方资料：

- 概述：https://developer.fnnas.com/api/overview/
- 调用方式：https://developer.fnnas.com/api/calling/
- 平台配置：https://developer.fnnas.com/api/platform-config/
- 授权与文件概览：https://developer.fnnas.com/api/authorization/overview/
- 应用共享授权：https://developer.fnnas.com/api/authorization/shared-access/
- 用户个人授权：https://developer.fnnas.com/api/authorization/user-access/
- 文件权限检查：https://developer.fnnas.com/api/authorization/file-acl/
- 路径转换：https://developer.fnnas.com/api/authorization/path-convert/
- 页面路由：https://developer.fnnas.com/api/page/routing/
- 页面交互：https://developer.fnnas.com/api/page/ui/
- 错误码：https://developer.fnnas.com/api/error-codes/
- SDK：https://www.npmjs.com/package/@trimjs/web-app
- 统一网关：https://developer.fnnas.com/docs/core-concepts/gateway-registration/

仓库依据：

- `docs/architecture.md`
- `docs/api-contract.md`
- `docs/fpk-packaging.md`
- `docs/future-development-plan.md`
- `packaging/fnos/manifest.template`
- `packaging/fnos/config/resource`
- `server/src/storage/mod.rs`
- `server/src/api/storage.rs`
