# FPK 打包说明

## 作用

这份文档只说明 **如何构建和定位 FPK 产物**，以及最小调试 / 排障入口。

它不记录项目阶段状态，也不承担运行时能力说明；其中命令、路径、产物命名和 manifest 约定必须与仓库脚本保持一致。

## 已查证约束

截至 2026-07-06，当前 FPK 打包约束以飞牛官方文档和本仓库本地验证为准：

- 官方 Manifest 文档明确了 `platform=x86|arm|all`、`os_min_version`、`service_port` 等字段，但**没有文档化 `arch` 字段**。当前仓库仍保留 x86 staging 中的 `arch = x86_64`，直到官方资料或实机验证证明可删。
- 官方应用框架文档列出了 `cmd/main`、`install_*`、`upgrade_*`、`uninstall_*`、`config_*` 生命周期脚本。
- 使用**当前已验证版本** `fnpack 1.2.1` 创建最小工程并在本地验证后确认：
  - 缺少 `cmd/main`、`install_*`、`upgrade_*`、`uninstall_*` 时，`fnpack build` 会报告 `Required file ... is missing`。
  - 缺少 `config_init` 或 `config_callback` 时，`fnpack build` 仍可成功。
  - `fnpack build` 在打印 `Packing failed` 时**仍可能返回退出码 0**，因此仓库构建脚本必须额外校验产物和日志，不能只信退出码。
- `config_callback` 当前承担授权目录快照同步职责，不纳入删除候选；`config_init` 只有在完成配置流程验证后才可评估是否移除。

如果后续升级 `fnpack`，需要重新验证至少以下行为是否仍成立：

- 缺少哪些生命周期脚本会导致打包失败；
- `fnpack build` 失败时退出码是否可靠；
- `--directory` 模式下 `.fpk` 产物写入位置；
- x86 staging 中 `arch = x86_64` 是否仍有必要保留。

相关官方资料：

- Manifest：https://developer.fnnas.com/docs/core-concepts/manifest/
- 应用框架 / 生命周期：https://developer.fnnas.com/docs/core-concepts/framework/
- fnpack：https://developer.fnnas.com/docs/cli/fnpack/

## 当前产物

默认命令会同时生成 x86 与 ARM 两个 FPK，`<version>` 来自核心版本源；Release workflow 会校验 `package.json`、`server/Cargo.toml` 与 `packaging/fnos/manifest.template` 保持一致：

- x86：`packaging/fnos/dist/motrix.fnos_<version>_x86.fpk`
- ARM：`packaging/fnos/dist/motrix.fnos_<version>_arm.fpk`

对应 server 二进制：

- x86：`server/target/x86_64-unknown-linux-gnu/release/motrix-fnos-server`
- ARM：`server/target/aarch64-unknown-linux-gnu/release/motrix-fnos-server`

设备架构必须匹配：

- `x86_64` 设备安装 x86 包
- `aarch64` / `arm64` 设备安装 ARM 包

## 常用命令

安装依赖：

```bash
rtk pnpm install
```

只做预组装，不执行 `fnpack build`：

```bash
rtk pnpm run build:fpk:prepare
```

同时构建 x86 与 ARM：

```bash
rtk pnpm run build:fpk
```

只构建 x86：

```bash
rtk pnpm run build:fpk:x64
```

只构建 ARM：

```bash
rtk pnpm run build:fpk:arm64
```

清理本地构建输出和 staged 产物：

```bash
rtk pnpm run clean
```

如需先查看会删除哪些内容：

```bash
rtk pnpm run clean:dry-run
```

## 打包目录

当前 FPK 主目录：

```text
packaging/fnos/
```

关键内容：

- `manifest.template`：源码态 manifest 模板输入
- `.stage/<target>/manifest`：预组装后的真实 manifest
- `cmd/`：启动、停止、状态脚本
- `config/`：资源与权限声明
- `app/bin/`：server 与 `aria2-next`
- `app/ui/dist/`：Web UI 静态资源
- `app/data/`：运行时数据目录
- `.stage/`：预组装后的打包目录
- `dist/`：最终输出的 `.fpk`

约定：

- 源码态 `packaging/fnos/` **不是**可直接执行 `fnpack build` 的安全输入目录；真实打包输入由 `build:fpk:prepare` / `build:fpk*` 生成到 `.stage/<target>/`。
- 如需手动检查 manifest、入口配置、端口配置或生命周期脚本，请检查 `.stage/x86/` 或 `.stage/arm/`，不要直接在源码态目录执行 `fnpack build`。
- `dist/`、`.stage/`、`app/bin/` 中构建脚本放置的 server / Aria2 二进制、`app/ui/dist/`、`dist/*.fpk` 和 stage 目录内产物都是本地生成产物，不应作为源码态内容长期保留。
- `assets/aria2/aria2-next-*` 是当前 `scripts/stage-aria2-sidecar.mjs` 使用的 sidecar 源资产，不是无用产物；只有未来改成下载缓存或发布资产拉取模式后，才可重新评估是否从仓库移除。

## 本地调试

先执行：

```bash
rtk pnpm run build:fpk:prepare
```

需要检查预组装结果时，查看：

```text
packaging/fnos/.stage/x86/
packaging/fnos/.stage/arm/
```

本地调试源码态脚本时，再执行：

```bash
rtk packaging/fnos/cmd/start
rtk packaging/fnos/cmd/status
rtk packaging/fnos/cmd/stop
```

常看两个位置：

- 日志：`packaging/fnos/app/data/logs/server.log`
- PID：`packaging/fnos/app/data/run/motrix-fnos-server.pid`

## 最小排障

- 安装失败：先检查包架构是否与设备一致
- 启动失败：先看 `server.log`
- Web UI 打不开：先看 `cmd/status` 和 `service_port`
- 下载失败：先看保存目录权限、Aria2 sidecar 和诊断日志
- 升级后任务或设置丢失：确认 `cmd/uninstall_callback` 默认保留 `TRIM_PKGVAR`，且未收到卸载向导删除数据变量
- 卸载后重装仍有旧任务：这是默认保留数据的预期行为；如需完全清理，卸载时开启“同时删除 Motrix 应用数据”

## 数据保留与卸载向导

fnOS 会在卸载时保留应用 `var` 类用户数据目录；本项目也以保留用户数据为默认策略：

- 升级必须保留 `TRIM_PKGVAR` 中的 SQLite、设置、JSON-RPC 密钥、Aria2 session 和日志。
- 卸载默认保留 `TRIM_PKGVAR`，便于后续重装继续使用原任务和设置。
- 只有卸载向导 `MOTRIX_FNOS_DELETE_APP_DATA` 被用户明确开启时，`cmd/uninstall_callback` 才会清理 `TRIM_PKGVAR`。
- 清理范围仅限 Motrix 应用私有数据；用户下载目录和已下载文件不在清理范围内。
- 卸载向导的 `switch` 初始值必须使用字符串 `"false"`；当前 `fnpack` 的 wizard schema 将 `items.initValue` 解析为 string，使用 JSON 布尔值会导致打包失败。

## 生命周期实机验证矩阵

下列项目用于 P6 及后续发布前的 fnOS 实机验证；未完成的项不得在文档中宣称“已验证通过”。

| 场景 | 操作 | 预期结果 | 重点观察 |
| --- | --- | --- | --- |
| 安装 | 安装匹配架构的 `.fpk` | 应用中心安装成功 | 安装界面报错、`TRIM_TEMP_LOGFILE`、应用中心任务日志 |
| 启动 | 在应用中心或 `appcenter-cli start` 启动 | 服务进入运行中，Web UI 可打开 | `cmd/status`、`server.log`、监听端口 |
| 停止 | 在应用中心或 `appcenter-cli stop` 停止 | 服务退出，状态变为未运行 | `cmd/status`、PID 文件是否清理 |
| 配置变更 | 在“应用设置”修改授权目录并保存 | `config_callback` 重新同步 accessible paths | `app/data/accessible-paths.json`、`server.log`、配置保存日志 |
| 升级 | 安装旧版本后升级到新包 | 数据与配置保留，服务可重新启动 | 升级界面日志、任务数据、`server.log` |
| 卸载（默认） | 卸载应用且不勾选删除数据 | `TRIM_PKGVAR` 应用数据保留，不删除用户下载文件 | 卸载向导选项、`cmd/uninstall_callback` 日志、`TRIM_PKGVAR` 内容 |
| 卸载（删除数据） | 卸载应用并勾选“同时删除 Motrix 应用数据” | 仅清理 `TRIM_PKGVAR` 内的 Motrix 应用数据，不删除用户下载文件 | `cmd/uninstall_callback` 日志、数据库/设置/session/log 是否被清理 |

建议实机验证命令：

```bash
appcenter-cli install-fpk <package>.fpk
appcenter-cli start motrix.fnos
appcenter-cli stop motrix.fnos
appcenter-cli list
```

如涉及向导或交互式配置，优先使用应用中心界面完成；`appcenter-cli` 更适合重复安装和脚本化验证。

## 相关文档

- 长期架构：`docs/architecture.md`
- 阶段状态：`docs/development-plan.md`
- 接口契约：`docs/api-contract.md`

## GitHub Release 在线打包

仓库提供 `Release FPK` workflow：

- 推送 `v*` tag 时自动构建 x86 与 ARM FPK，并创建 / 更新 GitHub Release。
- 也可以在 GitHub Actions 页面手动运行 workflow，默认使用 `package.json` / `server/Cargo.toml` / `packaging/fnos/manifest.template` 中一致的版本号生成 tag。
- workflow 会上传：
  - `motrix.fnos_<version>_x86.fpk`
  - `motrix.fnos_<version>_arm.fpk`
  - `SHA256SUMS.txt`

发布前必须确认 `package.json`、`server/Cargo.toml`、`packaging/fnos/manifest.template` 三处版本一致；workflow 会在版本不一致时失败。
