# 运行时生命周期修复完善计划

> 本文档基于 `docs/runtime-lifecycle-and-aria2-strategy.md` 的已实现能力，记录后续需要补强的三个方向：本应用 sidecar 组合识别、前端退出轮询协调、Aria2 session 恢复。本文是修复完善计划，不改变 `docs/architecture.md` 的架构边界。

## 摘要

当前运行时生命周期已经完成主要闭环：窗口关闭隐藏、明确退出暂停任务、停止 App 管理的 Aria2、启动时清理本应用残留 sidecar，并避免异常退出后自动继续下载。

后续仍建议补强以下能力：

1. 更严格识别本应用残留 sidecar，降低 PID 复用或外部进程误判风险。
2. 退出时通知前端停止任务轮询和操作，减少退出过程中的前后端竞态。
3. 引入 Aria2 session 文件，提升断点续传和复杂任务恢复可靠性。

## 任务一：完善本应用 sidecar 组合识别

### 目标

清理残留 Aria2 时，不仅依赖运行态记录中的 PID、端口、source、secret 非空，还要结合进程命令行、sidecar 路径或应用数据目录标识进行组合判断。只有足够确认该进程属于 Motrix FNOS，才允许终止。

### 小任务 1.1：扩展运行态记录字段

- 在 Aria2 运行态记录中补充可用于识别的字段，例如：
  - sidecar 可执行文件路径或 sidecar 名称。
  - app data 目录下的 session/input/log 路径。
  - 启动参数摘要或可验证标识。
- 保留现有字段兼容：PID、端口、RPC secret、endpoint、binary source。
- 对旧版本运行态文件做兼容读取，缺失字段时按低置信度处理。

验证：

- 新增运行态序列化/反序列化单元测试。
- 旧格式运行态记录仍能读取，不 panic。
- `rtk cargo test --manifest-path src-tauri/Cargo.toml`

### 小任务 1.2：实现跨平台进程命令行读取

- macOS/Linux：通过 `ps -p <pid> -o command=` 或等价方式读取命令行。
- Windows：通过 PowerShell 或系统 API 查询进程命令行。
- 查询失败时返回明确结果，不直接认定为本应用进程。
- 日志中记录查询失败原因，便于排障。

验证：

- 抽象纯函数解析命令行，新增单元测试覆盖：
  - 包含 sidecar 名称。
  - 包含 `--rpc-listen-port=<port>`。
  - 包含 `--rpc-secret=<secret>`。
  - 包含 app data session/log 路径。
  - 不相关命令行。
- `rtk cargo test --manifest-path src-tauri/Cargo.toml`

### 小任务 1.3：建立组合置信度判定规则

- 定义 sidecar 归属判定规则，例如满足以下多项才可清理：
  - PID 与运行态记录一致。
  - 端口与运行态记录一致。
  - binary source 为内置 sidecar。
  - 命令行包含 sidecar 名称或路径。
  - 命令行包含旧 RPC secret。
  - 命令行包含 app data 下 session/input/log 路径。
- 对 PID 复用、命令行不匹配、secret 不匹配的情况返回 `ExternalOrUnknown`。
- 保持外部 aria2 或其他程序占用端口时：跳过、不连接、不杀。

验证：

- 新增判定规则单元测试。
- 覆盖“运行态记录匹配但命令行不匹配”的 PID 复用风险。
- 覆盖“外部 aria2 占用 6800 时切换备用端口”。
- `rtk cargo test --manifest-path src-tauri/Cargo.toml`

### 小任务 1.4：接入启动前残留清理流程

- `select_rpc_port_with_saved_runtime` 使用新的组合判定结果。
- 确认为本应用残留 sidecar 时才清理。
- 确认为本应用残留但清理失败时，不启动新 sidecar，避免旧进程继续下载。
- 不确定归属时跳过该端口。

验证：

- Rust 单元测试覆盖清理成功、清理失败、外部占用跳过。
- 手动验证：模拟外部进程占用 6800，应用应切换到备用端口。
- 手动验证：模拟本应用残留 sidecar，应用应清理后复用端口。

## 任务二：前端退出轮询协调

### 目标

后端进入统一退出流程时，显式通知前端停止任务轮询和禁用任务操作，避免退出过程中前端继续 `list_download_tasks` 或触发任务控制命令，减少状态保存竞态。

这不是用户通知，也不只是提示语；它是前后端退出协调机制。可选显示轻量“正在退出”状态。

### 小任务 2.1：定义退出事件协议

- 后端统一退出流程开始时向前端 emit 事件，例如：
  - `runtime://exiting`
  - payload 包含 reason、timestamp。
- 事件只表示“退出流程已开始”，不承诺退出一定成功。
- 前端 service 层集中监听，不在组件里散落监听逻辑。

验证：

- TypeScript 类型检查通过。
- Rust 编译通过。

### 小任务 2.2：Pinia 增加退出中状态

- 在应用或任务 store 中增加 `isAppExiting` / `isRuntimeExiting`。
- 收到退出事件后设置为 true。
- 任务 store 暴露只读状态给组件使用。

验证：

- `rtk pnpm run typecheck`

### 小任务 2.3：停止任务轮询

- `useTaskPolling` 监听退出状态或退出事件。
- 收到退出事件后：
  - clearInterval。
  - 阻止后续 refresh。
  - 避免并发 refresh 结果覆盖退出前状态。
- 组件卸载时仍保留现有清理逻辑。

验证：

- 手动验证退出时日志不再出现前端触发的任务刷新。
- `rtk pnpm run typecheck`

### 小任务 2.4：禁用退出中的任务操作

- 任务操作按钮在退出中禁用。
- 删除、暂停、继续、重新下载等操作在退出中不再发起 command。
- 可选显示轻量提示：“应用正在退出，请稍候”。

验证：

- `rtk pnpm run typecheck`
- 手动验证退出过程中按钮不可继续点击。

### 小任务 2.5：后端 command 增加退出中保护

- 对任务控制 command 增加 `is_exiting` 检查。
- 退出中返回明确错误，例如“应用正在退出，不能执行任务操作”。
- 避免前端未及时停止轮询或操作时影响退出流程。

验证：

- Rust 单元测试或窄范围 command 逻辑测试。
- `rtk cargo test --manifest-path src-tauri/Cargo.toml`

## 任务三：引入 Aria2 session 恢复能力

### 目标

在 SQLite 业务状态和 `.aria2` 断点控制文件之外，引入 Aria2 原生 session 文件，提升未完成任务、复杂任务、多文件任务、BT/磁力任务的恢复可靠性。

SQLite 仍作为 UI 和业务状态来源；Aria2 session 作为下载引擎恢复来源；`.aria2` 文件继续作为文件级断点辅助。

### 小任务 3.1：设计 session 文件路径

- 在应用数据目录下创建 Aria2 runtime 子目录，例如：
  - `<app_data>/aria2/aria2.session`
  - `<app_data>/aria2/aria2.log`（可选）
- 路径写入运行态记录，用于 sidecar 归属判断。
- 确保目录创建失败有明确错误和日志。

验证：

- 新增路径生成单元测试。
- `rtk cargo test --manifest-path src-tauri/Cargo.toml`

### 小任务 3.2：启动 Aria2 时启用 session 参数

- 内置 sidecar 启动参数追加：
  - `--input-file=<session_path>`
  - `--save-session=<session_path>`
  - `--save-session-interval=30`
  - `--force-save=true`
- 保留既有强制参数：
  - `--enable-rpc=true`
  - `--rpc-listen-all=false`
  - `--rpc-listen-port=<actual_port>`
  - `--rpc-secret=<secret>`
  - `--no-conf=true`
- 启动时应避免 session 任务自动下载，可结合：
  - `--pause=true`
  - 或启动后立即 `pauseAll`。

验证：

- 更新 `process_args` 单元测试。
- `rtk cargo test --manifest-path src-tauri/Cargo.toml`

### 小任务 3.3：退出前保存 Aria2 session

- 统一退出流程中，在暂停未完成任务后调用 `aria2.saveSession`。
- RPC 不可用时记录 warn，不阻塞退出。
- 保存 session 应在停止 Aria2 前执行。

验证：

- 新增 JSON-RPC 请求构造测试。
- 手动验证退出日志包含 session 保存成功或失败原因。
- `rtk cargo test --manifest-path src-tauri/Cargo.toml`

### 小任务 3.4：启动后同步 SQLite 与 Aria2 session 恢复任务

- Aria2 启动并加载 session 后，从 Aria2 拉取任务状态。
- 将 Aria2 当前 GID 与 SQLite 任务进行匹配。
- 优先匹配：
  - URL。
  - 保存目录。
  - 文件名或文件路径。
  - 原 GID（如果仍有效）。
- 匹配成功后更新 SQLite 中的 GID、状态、进度和文件路径。
- 匹配失败的 session 任务应记录日志，先不自动创建陌生 UI 任务，避免误导。

验证：

- 新增匹配规则单元测试。
- 手动验证异常退出后重启：任务显示暂停，点击继续可断点续传。
- `rtk cargo test --manifest-path src-tauri/Cargo.toml`

### 小任务 3.5：确保启动后默认暂停，不自动下载

- 无论 session 中任务是什么状态，应用启动后默认不自动下载未完成任务。
- 可以通过 `--pause=true` 保证 Aria2 加载 session 时暂停。
- 后端启动兜底仍保留：SQLite 中 `active/pending` 先转为 `paused`。
- 用户点击“继续”后再恢复下载。

验证：

- 手动验证：下载中退出或强杀后重启，网络不会自动跑满，任务显示暂停。
- 手动验证：点击继续后从已有进度续传。

### 小任务 3.6：保留 SQLite 作为业务状态来源

- session 不替代 SQLite。
- SQLite 继续保存 UI 展示、历史、错误、任务元数据。
- session 只作为 Aria2 引擎恢复输入。
- 如果 session 文件损坏，应回退到 SQLite + URL 重新加入任务的现有恢复逻辑，并写日志。

验证：

- 手动删除或损坏 session 文件后，应用仍可启动。
- 旧 SQLite 任务仍可展示和操作。

## 推荐实施顺序

1. 先做任务一：sidecar 组合识别，降低误杀和残留风险。
2. 再做任务三：Aria2 session，因为它会影响 sidecar 启动参数和残留识别字段。
3. 最后做任务二：前端退出轮询协调，减少退出期间竞态并补齐体验。

如果优先解决断点续传和异常退出恢复，应先做任务三；如果优先解决进程安全边界，应先做任务一。

## 总体验收标准

- 正常退出：任务保存为暂停态，Aria2 sidecar 被停止，运行态记录按结果正确清理或保留。
- 异常退出：下次启动先清理确认属于本应用的残留 sidecar，不误杀外部进程。
- 启动恢复：未完成任务默认暂停，不自动下载。
- 点击继续：优先基于 Aria2 session 和 `.aria2` 文件断点续传。
- 外部 aria2 或其他程序占用端口：跳过，不连接、不杀。
- 退出过程中：前端不再继续轮询或发起任务操作。
